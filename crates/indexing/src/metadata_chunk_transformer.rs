use std::{collections::HashMap, fs, path::Path};

use ai_agent_common::{Language, LanguageIter};
use strum::IntoEnumIterator;
use anyhow::{anyhow, Result, Context};
use async_trait::async_trait;
use repo_root::{projects::GitProject, RepoRoot};
use serde_json::{json, Value};
use swiftide::{indexing::{Metadata, Node, TextNode, Transformer}, traits::WithIndexingDefaults};
use tree_sitter::{{Language as TSLanguage}, Node as TsNode, Parser, Query, QueryCursor, Tree, QueryMatches, StreamingIterator, TextProvider};
use tree_sitter;

use tree_sitter_rust;
use tree_sitter_python;
use tree_sitter_javascript;
use tree_sitter_typescript;
use tree_sitter_java;
use tree_sitter_c;
use tree_sitter_cpp;
use tree_sitter_go;
use tree_sitter_haskell;
use tree_sitter_lua;
use tree_sitter_yaml;
use tree_sitter_bash;
use tree_sitter_html;
use tree_sitter_json;
use tree_sitter_ruby;
use tree_sitter_asciidoc;
use tree_sitter_xml;
use tree_sitter_md;
use tree_sitter_yarn;

use crate::metadata_transformer::ExtractMetadataTransformer;

#[derive(Clone)]
pub struct ExtractMetadataChunkTransformer{
}

impl WithIndexingDefaults for ExtractMetadataChunkTransformer {}

impl ExtractMetadataChunkTransformer {
    pub fn new() -> Self {
        Self {}
    }

    fn parse_tstree(&self, contents: Option<&Value>, path: &Path) -> Result<Tree>{

        let language = {
            let ext : &str = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            ExtractMetadataTransformer::get_language_from_extension(ext)
        };
        match contents {
            Some(content) => {
                let parsed_content = serde_json::to_string(content)?;
                match language{
                    Some(lang) =>{
                        let mut parser = Parser::new();
                        parser.set_language(&lang).expect("Failed to set language");

                        parser.parse(parsed_content,None).context("Failed to parse AST tree for file")

                    },
                    None => Err(anyhow!(""))
                }
            },
            None => Err(anyhow!(""))
        }

    }

    /// Map file extension to tree-sitter Language
    pub fn get_language_from_extension(extension: &str) -> Option<TSLanguage> {
        match extension.to_lowercase().as_str() {
            "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
            "py" => Some(tree_sitter_python::LANGUAGE.into()),
            "js" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "ts" => Some(unsafe { tree_sitter_typescript::LANGUAGE_TYPESCRIPT }.into()),
            "tsx" => Some(unsafe { tree_sitter_typescript::LANGUAGE_TSX.into() }),
            "java" => Some(tree_sitter_java::LANGUAGE.into()),
            "c" => Some(tree_sitter_c::LANGUAGE.into()),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" => Some(tree_sitter_cpp::LANGUAGE.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            "hs" => Some(tree_sitter_haskell::LANGUAGE.into()),
            "lua" => Some(tree_sitter_lua::LANGUAGE.into()),
            "yaml" | "yml" => Some(tree_sitter_yaml::LANGUAGE.into()),
            "sh" => Some(tree_sitter_bash::LANGUAGE.into()),
            "html" => Some(tree_sitter_html::LANGUAGE.into()),
            "json" => Some(tree_sitter_json::LANGUAGE.into()),
            "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
            "adoc" => Some(tree_sitter_asciidoc::language().into()),
            "xml" => Some(tree_sitter_xml::LANGUAGE_XML.into()),
            "md" => Some(tree_sitter_md::LANGUAGE.into()),
            "yarn" => Some(tree_sitter_yarn::LANGUAGE.into()),
            _ => None,
        }
    }

    // Extract function or method definitions (kinds vary per language)
    fn extract_definitions(&self, ts_node: TsNode, source: &[u8]) -> Vec<String> {
        let def_kinds = [
            "function_item",
            "method_definition",
            "function_declaration",
            "function",
            "def", // python
            "method",
        ];
        let mut defs = Vec::new();
        let mut cursor = ts_node.walk();
        let mut to_visit = vec![ts_node];

        while let Some(node) = to_visit.pop() {
            if def_kinds.contains(&node.kind()) {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(text) = name_node.utf8_text(source) {
                        defs.push(text.to_string());
                    }
                }
            }
            for child in node.children(&mut cursor) {
                to_visit.push(child);
            }
        }
        defs
    }

    // Extract use/import declarations as dependencies (language grammar dependent)
    fn extract_dependencies(&self, ts_node: TsNode, source: &[u8]) -> Vec<String> {
        let dep_kinds = [
            "use_declaration",      // Rust
            "import_statement",     // Python, JavaScript, TypeScript
            "import_declaration",   // JavaScript, TypeScript
            "package_clause",       // Go
            "import_list",          // Java
        ];
        let mut deps = Vec::new();
        let mut cursor = ts_node.walk();

        for child in ts_node.children(&mut cursor) {
            if dep_kinds.contains(&child.kind()) {
                if let Ok(text) = child.utf8_text(source) {
                    // Heuristic: extract first identifier / crate/package name
                    let dep = text.trim().split_whitespace().nth(1).unwrap_or("");
                    let dep_name = dep.split(&[',', ';', '{', '(', ' '][..]).next().unwrap_or("");
                    if !dep_name.is_empty() && !deps.contains(&dep_name.to_string()) {
                        deps.push(dep_name.to_string());
                    }
                }
            }
        }
        deps
    }

    // Extract identifiers for references
    fn extract_references(&self, ts_node: TsNode, source: &[u8]) -> Vec<String> {
        let mut refs = Vec::new();
        let mut cursor = ts_node.walk();
        let mut to_visit = vec![ts_node];

        while let Some(node) = to_visit.pop() {
            if node.kind() == "identifier" {
                if let Ok(text) = node.utf8_text(source) {
                    if !refs.contains(&text.to_string()) {
                        refs.push(text.to_string());
                    }
                }
            }
            for child in node.children(&mut cursor) {
                to_visit.push(child);
            }
        }
        refs
    }

    /// Finds all nodes in `root_node` that reference the given `subnode`
    /// (e.g., all calls to the function represented by `subnode`).
    ///
    /// # Parameters
    /// - `root_node`: The root node representing the entire parsed document tree.
    /// - `source`: The source code as bytes slice used for parsing (needed for extracting text).
    /// - `subnode`: The node representing the subnode to find references to (e.g. function definition).
    ///
    /// # Returns
    /// A vector of nodes representing the call sites or references to the subnode.
    fn find_references_to_subnode(&self, root_node: TsNode, source: &[u8], subnode: TsNode) -> Option<Vec<String>> {
        // Extract the identifying text of the subnode, e.g. the function name
        let name_node_option = subnode.child_by_field_name("name");
        match name_node_option {
            Some(name_node) =>{
                let function_name = name_node.utf8_text(source).expect("Failed to extract name text");

                let language = root_node.language();

                // Query for call expressions where the called function's identifier matches the subnode's name
                let query_str = format!(
                    r#"
                    ; Rust: Function, Struct, Enum, Trait references
                    (
                      (call_expression function: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (struct_expression (identifier) @ref_struct) (#eq? @ref_struct "{}")
                    )
                    (
                      (type_identifier) @ref_type (#eq? @ref_type "{}")
                    )
                    (
                      (trait_reference) @ref_trait (#eq? @ref_trait "{}")
                    )

                    ; Python: Function call, Class instantiation
                    (
                      (call function: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (class_definition name: (identifier) @ref_class) (#eq? @ref_class "{}")
                    )

                    ; JavaScript: Function call, Class reference
                    (
                      (call_expression function: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (class_declaration name: (identifier) @ref_class) (#eq? @ref_class "{}")
                    )

                    ; Java: Method call, Class reference, Interface reference
                    (
                      (method_invocation name: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (class_declaration name: (identifier) @ref_class) (#eq? @ref_class "{}")
                    )
                    (
                      (interface_declaration name: (identifier) @ref_interface) (#eq? @ref_interface "{}")
                    )

                    ; C: Function call, Struct reference
                    (
                      (call_expression function: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (struct_specifier name: (type_identifier) @ref_struct) (#eq? @ref_struct "{}")
                    )

                    ; Go: Function call, Struct, Interface references
                    (
                      (call_expression function: (identifier) @ref_func) (#eq? @ref_func "{}")
                    )
                    (
                      (type_spec name: (type_identifier) @ref_struct) (#eq? @ref_struct "{}")
                    )
                    (
                      (interface_type) @ref_interface
                    )
                    "#,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                    function_name,
                );

                let query = Query::new(&language, &query_str)
                    .expect("Failed to compile tree-sitter query");
                let mut cursor = QueryCursor::new();
                let mut references = Vec::new();
                let mut matches = cursor.matches(&query, root_node, source);

                while let Some(mat) = matches.next() {
                    for cap in mat.captures {
                            match query.capture_names()[cap.index as usize] {
                                "ref_func" | "ref_struct" | "ref_class" | "ref_interface" | "ref_trait" => {
                                    references.push(cap.node.to_string());
                                }
                                _ => {}
                            }
                        }
                }

                Some(references)
            },
            None => None
        }
    }

    // Language-specific query mapping
    fn get_call_query_for_language(&self, language: &String) -> Option<&'static str> {
        let languages: Vec<String> = Language::iter().map(|l| l.to_string())
            .filter(|l| l !="Markdown")
            .collect();
        if languages.contains(language) {
                Some("(call_expression)")
            }
            else {
                None
        }
    }

    fn find_all_calls_in_node(
        &self,
        parent_node: TsNode,
        source: &[u8],
        language_name: &String,
        path: &Path,
    ) -> Option<Vec<String>> {
        // Check if the language supports call expressions
        let query_str = match self.get_call_query_for_language(language_name) {
            Some(q) => q,
            None => {
                // Return empty vector for languages without call expressions
                return Some(Vec::new());
            }
        };

        let language = {
            let ext : &str = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            ExtractMetadataTransformer::get_language_from_extension(ext)
        };
        match language{
            Some(lang) => {
                match Query::new(&lang, query_str){
                    Ok(q) =>{
                        let mut cursor = QueryCursor::new();
                        let mut calls = Vec::new();
                        let mut matches = cursor.matches(&q, parent_node, source);

                        while let Some(mat) = matches.next() {
                            for cap in mat.captures {
                                calls.push(cap.node.to_string());
                            }
                        }

                        Some(calls)
                    },
                    Err(_) => None
                }
            },
            None => None
        }

    }

    fn parse_string_to_node(&self, source_code: &str, path: &Path) -> Result<Tree> {

        let language = {
            let ext : &str = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            ExtractMetadataTransformer::get_language_from_extension(ext)
        }.context("Failed to infer language type from extension")?;
        let mut parser = Parser::new();
        parser.set_language(&language).expect("Error loading grammar");
        parser.parse(source_code, None).context("Failed to parse AST")
    }
}

fn filter_fields_out(metadata: &Metadata, fields: &[&str]) -> HashMap<String, Value> {
    metadata
        .iter()
        .filter(|(key, _)| !fields.contains(&key.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn create_filtered_metadata(metadata: &Metadata, fields: &[&str]) -> Metadata {
    let mut new_metadata = Metadata::default();

    for field in fields {
        if let Some(value) = metadata.get(field) {
            new_metadata.insert(field.to_string(), value.clone());
        }
    }

    new_metadata
}

#[async_trait]
impl Transformer for ExtractMetadataChunkTransformer {
    type Input = String;
    type Output = String;

    async fn transform_node(&self, mut node: TextNode) -> anyhow::Result<TextNode> {
        let original_content = node.metadata.get("original_content");
        let language = node.metadata.get("language");
        let lang_value =  language.context("Failed to retrieve langauge from previous chunk")?;
        let lang = serde_json::to_string(&lang_value)?;
        let tree = self.parse_tstree(original_content, &node.path)?;
        let root = tree.root_node();
        let subtree = self.parse_string_to_node(&node.chunk, &node.path)?;

        // let references = self.extract_references(root, &node.chunk.as_bytes());
        let references_option = self.find_references_to_subnode(root, &node.chunk.as_bytes(), subtree.root_node());
        let calls_options = self.find_all_calls_in_node(root, &node.chunk.as_bytes(), &lang, &node.path);

        let definitions = self.extract_definitions(root, &node.chunk.as_bytes());
        let imports = self.extract_dependencies(root, &node.chunk.as_bytes());
        let kind = root.kind();
        let parent_node = root.kind().to_string();
        let filtered_hashmap = filter_fields_out(&node.metadata, &vec!["original_content"]);
        let mut filtered_metadata = Metadata::default();
        filtered_metadata.extend(filtered_hashmap);
        node.metadata = filtered_metadata;
        if let Some(references) = references_option {
            node.metadata.insert("called_by", json!(references));
        }
        if let Some(calls) = calls_options {
            node.metadata.insert("calls", json!(calls));
        }
        node.metadata.insert("definitions", json!(definitions));
        node.metadata.insert("kind", json!(kind));
        node.metadata.insert("imports", json!(imports));
        node.metadata.insert("parent_node", json!(parent_node));

        Ok(node)
    }
}
