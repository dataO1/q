use std::{collections::HashMap, fs, path::Path};

use ai_agent_common::{Definition, DefinitionBuilder, Language, LanguageIter, StructureContextFragment, StructureContextFragmentBuilder};
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


    pub fn get_query_for_language(&self,language: &str) -> &'static str {
        match language.to_lowercase().as_str() {
            "rust" => r#"
                (function_item name: (identifier) @function.name)
                (impl_item) @impl
                (mod_item name: (identifier) @mod.name)
                (struct_item name: (type_identifier) @struct.name)
                (trait_item name: (identifier) @trait.name)
                (enum_item name: (identifier) @enum.name)
            "#,

            "python" => r#"
                (function_definition name: (identifier) @function.name)
                (class_definition name: (identifier) @class.name)
            "#,

            "javascript" | "typescript" => r#"
                (function_declaration name: (identifier) @function.name)
                (function_expression name: (identifier) @function.name)
                (class_declaration name: (identifier) @class.name)
                (method_definition name: (property_identifier) @method.name)
                (interface_declaration name: (identifier) @interface.name)
            "#,

            "java" => r#"
                (method_declaration name: (identifier) @method.name)
                (class_declaration name: (identifier) @class.name)
                (interface_declaration name: (identifier) @interface.name)
            "#,

            "go" => r#"
                (function_declaration name: (identifier) @function.name)
                (method_declaration name: (identifier) @method.name)
                (type_declaration name: (type_identifier) @type.name)
                (interface_type name: (identifier) @interface.name)
            "#,

            _ => "",
        }
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


    fn filter_fields_out(&self, metadata: &Metadata, fields: &[&str]) -> HashMap<String, Value> {
        metadata
            .iter()
            .filter(|(key, _)| !fields.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    fn print_node_tree(&self, node: TsNode, source: &[u8], indent: usize) {
        let indent_str = "  ".repeat(indent);

        // Get the node's source text
        let text = node.utf8_text(source).unwrap_or("<invalid utf8>");

        // Print current node info
        println!(
            "{}[{}] kind: '{}', is_named: {}, text: '{}'",
            indent_str,
            node.id(),
            node.kind(),
            node.is_named(),
            text.lines().next().unwrap_or("").chars().take(50).collect::<String>()
        );

        // Print field names for children
        let mut cursor = node.walk();
        let field_names: Vec<_> = (0..node.child_count())
            .filter_map(|i| node.field_name_for_child(i as u32))
            .collect();

        if !field_names.is_empty() {
            println!("{}  fields: {:?}", indent_str, field_names);
        }

        // Recursively print children
        for child in node.children(&mut cursor) {
            self.print_node_tree(child, source, indent + 1);
        }
    }

    pub fn extract_named_definitions(
        &self,
        root_node: TsNode,
        language: &tree_sitter::Language,
        source_code: &[u8],
        query_source: &str,
    ) -> anyhow::Result<Vec<StructureContextFragment>> {

        let query = Query::new(language, query_source)?;
        let mut query_cursor = QueryCursor::new();

        let mut results = Vec::new();
        let mut matches_iter = query_cursor.matches(&query, root_node, source_code);
        while let Some(m) = matches_iter.next() {
            for capture in m.captures {
                let node = capture.node;
                let capture_name = &query.capture_names()[capture.index as usize];

                // Get node text (e.g. function/class name)
                let node_text = node.utf8_text(source_code).unwrap_or("");
                    let kind= node.parent().map_or(node.kind().to_string(), |p| p.kind().to_string());
                    let name= Some(node_text.to_string());
                    let line_start= node.start_position().row + 1;
                    let line_end= node.end_position().row + 1;
                    let structure = StructureContextFragmentBuilder::default().kind(kind).name(name).line_start(line_start).line_end(line_end).build()?;

                results.push( structure);
            }
        }

        Ok(results)
    }
}



#[async_trait]
impl Transformer for ExtractMetadataChunkTransformer {
    type Input = String;
    type Output = String;


    async fn transform_node(&self, mut node: TextNode) -> anyhow::Result<TextNode> {
        // parse trees and nodes
        let original_content = node.metadata.get("original_content");
        let lang = node.metadata.get("language");
        let lang_value =  lang.context("Failed to retrieve langauge from previous chunk")?;
        let language = serde_json::to_string(&lang_value)?;

        let ts_lang = {
            let ext : &str = node.path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            ExtractMetadataTransformer::get_language_from_extension(ext)
        }.unwrap();
        let tree = self.parse_tstree(original_content, &node.path)?;
        let root_node = tree.root_node();
        let chunk = &node.chunk;
        let bytes = node.chunk.as_bytes();
        let subtree = self.parse_string_to_node(chunk, &node.path)?;
        let subroot_node = subtree.root_node();

        // compute the relevant informatoin

        let kind = subroot_node.kind();
        let structures = self.extract_named_definitions(subroot_node,&ts_lang,bytes,self.get_query_for_language(&language))?;

        // filter out original_content of the super tree
        let filtered_hashmap = self.filter_fields_out(&node.metadata, &vec!["original_content"]);
        let mut filtered_metadata = Metadata::default();
        filtered_metadata.extend(filtered_hashmap);

        // set metadata
        node.metadata = filtered_metadata;
        node.metadata.insert("kind", json!(kind));
        node.metadata.insert("structures", json!(structures));
        node.metadata.insert("line_start", json!(subroot_node.start_position().row + 1));
        node.metadata.insert("line_end", json!(subroot_node.end_position().row +1));

        Ok(node)
    }
}
