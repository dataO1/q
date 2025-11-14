use std::{fs, path::Path}; use anyhow::{ anyhow, Result, Context };
use async_trait::async_trait;
use repo_root::{projects::GitProject, RepoRoot};
use serde_json::json;
use swiftide::{indexing::{TextNode, Transformer}, traits::WithIndexingDefaults};
use tree_sitter::{Language, Node as TsNode, Parser, Tree};

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

#[derive(Clone)]
pub struct ExtractMetadataTransformer{
    project_root: String,
}

impl WithIndexingDefaults for ExtractMetadataTransformer {}

impl ExtractMetadataTransformer {
    pub fn new(project_root: String) -> Self {
        Self {project_root}
    }


    fn get_tstree(&self, path: &Path) -> Result<Tree>{

        let lang = {
            let ext : &str = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            ExtractMetadataTransformer::get_language_from_extension(ext)
        };

        if lang.is_none() {
            // No language found - return node unchanged
            return Err(anyhow!("Failed to extract metadata for unknown language"));
        }

        let mut parser = Parser::new();
        parser.set_language(&lang.unwrap()).expect("Failed to set language");

        let contents = fs::read_to_string(path)?;

        parser.parse(contents,None).context("Failed to parse AST tree for file")
    }

    /// Map file extension to tree-sitter Language
    pub fn get_language_from_extension(extension: &str) -> Option<Language> {
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
}

#[async_trait]
impl Transformer for ExtractMetadataTransformer {
    type Input = String;
    type Output = String;

    async fn transform_node(&self, mut node: TextNode) -> Result<TextNode> {
        let tree = self.get_tstree(&node.path)?;
        let root = tree.root_node();
        let language = tree.language().name();
        let dependencies = self.extract_dependencies(root, &node.chunk.as_bytes());


        node.metadata.insert("original_content", json!(node.chunk));
        node.metadata.insert("language", json!(language));
        // node.metadata.insert("imports", json!(dependencies));
        node.metadata.insert("project_root", json!(self.project_root));

        Ok(node)
    }
}
