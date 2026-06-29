use std::fs::write;
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::{
            BuiltIn,
            filesystem::utils::validate_path,
        },
    }
};

/// params for mermaid flowchart
#[derive(Deserialize)]
struct Params {
    file_path: String,
    content:   String,
}

/// built-in tool
pub struct Mermaid;

impl Mermaid {
    /// new
    pub fn new() -> Self {
        Mermaid
    }
}

impl BuiltIn for Mermaid {
    /// get tool name
    fn name(&self) -> String {
        "mermaid".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Create a new html file or completely overwrite an existing html file with new mermaid diagram syntax. Renders Mermaid diagram syntax into a standalone HTML file for visualization in a web browser.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the html file to write to.",
                },
                "content": {
                    "type": "string",
                    "description": "The Mermaid diagram syntax string defining the chart, flow, sequence, or other diagram types to write to the html file.",
                },
            },
            "required": ["file_path", "content"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(&params.file_path.replace("\\", "/")), false)?;
        write(valid_path, mermaid_html(&params.content))?;
        Ok((format!("Successfully create mermaid flowchart: {}", params.file_path), None))
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        if is_en {
            Ok(Some(format!("Do you allow calling the mermaid tool to create {} or completely overwrite this file?{}", params.file_path, info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 mermaid 工具创建 {}，或覆盖已有的这个文件？{}", params.file_path, info.unwrap_or_default())))
        }
    }
}

/// mermaid 单html模版
const HTML: &str = r###"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta http-equiv="X-UA-Compatible" content="IE=edge" />
    <title>Mermaid</title>
    <link rel="icon" type="image/x-icon" href="data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0idXRmLTgiPz48IS0tIFVwbG9hZGVkIHRvOiBTVkcgUmVwbywgd3d3LnN2Z3JlcG8uY29tLCBHZW5lcmF0b3I6IFNWRyBSZXBvIE1peGVyIFRvb2xzIC0tPgo8c3ZnIHZlcnNpb249IjEuMSIgaWQ9ImRlc2lnbnMiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgeG1sbnM6eGxpbms9Imh0dHA6Ly93d3cudzMub3JnLzE5OTkveGxpbmsiIHZpZXdCb3g9IjAgMCAzMiAzMiIgeG1sOnNwYWNlPSJwcmVzZXJ2ZSI+CjxzdHlsZSB0eXBlPSJ0ZXh0L2NzcyI+Cgkuc2tldGNoeV9lZW57ZmlsbDojMTExOTE4O30KPC9zdHlsZT4KPHBhdGggY2xhc3M9InNrZXRjaHlfZWVuIiBkPSJNMzAuOTQzLDI0LjkzNGMtMC4wMTItMC40My0wLjA0OC0wLjg2LTAuMDM0LTEuMjg5YzAuMDE0LTAuNTE0LDAuMDU2LTEuMDI1LDAuMDktMS41MzcKCWMwLjAyOC0wLjQ0Mi0wLjM5Mi0wLjgxNC0wLjgxNS0wLjgxNGMtMC4wMzcsMC0wLjA2OCwwLjAxNi0wLjEwMywwLjAyMWMtMC4xMi0wLjA3OC0wLjI1My0wLjEzNS0wLjQwMi0wLjEyNwoJYy0wLjcxNCwwLjAzNC0xLjQzNSwwLjAwOC0yLjE0OS0wLjAwNmMtMC4zODMtMC4wMDktMC43NjQtMC4wMTEtMS4xNDYtMC4wMTdjMC4wMDItMC4yNzIsMC4wMTItMC41NDMsMC4wMjQtMC44MTUKCWMwLjAxOC0wLjA2OCwwLjA0MS0wLjEzMywwLjAzOC0wLjIwNmMtMC4wMTgtMC4zNzYtMC4wODYtMC43NDktMC4wOTgtMS4xMjdjLTAuMDA2LTAuMjE4LDAuMDE4LTAuNDQyLDAuMDQtMC42NgoJYzAuMDI2LTAuMjU0LDAuMDQ2LTAuNTEsMC4wNzItMC43NjdjMC4wMjItMC4yMTItMC4xMDQtMC40NTQtMC4yNDgtMC42Yy0wLjAwNi0wLjAwNi0wLjAxNi0wLjAwOS0wLjAyMi0wLjAxNQoJYy0wLjE0Ni0wLjE2OS0wLjM1LTAuMjg2LTAuNTc5LTAuMjg2Yy0wLjAxNSwwLTAuMDMsMC0wLjA0NSwwLjAwMmMtMC4zMjYsMC4wMi0wLjY1LDAuMDcyLTAuOTc3LDAuMQoJYy0wLjUxNCwwLjA0Mi0xLjAzMywwLjAzMi0xLjU0OSwwLjAzNmMtMC45NTcsMC4wMDgtMS45MTItMC4wMTYtMi44NjgtMC4wMzRjLTAuMDA0LTAuMDA1LTAuMDA1LTAuMDEtMC4wMS0wLjAxNQoJYy0wLjIzOC0wLjI1OC0wLjQ1OC0wLjUzNC0wLjY5Mi0wLjgwMWMtMC4xMjItMC4xMzgtMC4yNDQtMC4yNzYtMC4zOC0wLjRjLTAuMTE2LTAuMTA4LTAuMjQyLTAuMjA0LTAuMzYyLTAuMzA2CgljLTAuNjUxLTAuNTQ5LTEuMjQzLTEuMTY2LTEuODctMS43NDFjLTAuMDQ3LTAuNTUtMC4wNTMtMS4xMDQtMC4wNjgtMS42NTZjMC4yODIsMC4wMDIsMC41NjQtMC4wMDEsMC44NDYsMAoJYzAuODE3LDAuMDAyLDEuNjMxLTAuMDA2LDIuNDUtMC4wMjZjMC40Ni0wLjAxMiwwLjkyMSwwLjAwNiwxLjM4MywwLjAxNGMwLjIzMiwwLjAwNiwwLjQ2NiwwLjAwMiwwLjY5OCwwLjAwMgoJYzAuMTc0LDAsMC4zNDQtMC4wMjgsMC41MTgtMC4wMzRjMC40Ny0wLjAxNiwwLjg2My0wLjM4LDAuODYzLTAuODYzYzAtMC4xMDgtMC4wMjMtMC4yMTYtMC4wNjQtMC4zMTcKCWMtMC4wMzktMC40OTMtMC4wMzQtMC45ODctMC4wNDctMS40ODJjLTAuMDE0LTAuNDktMC4wNzYtMC45NzctMC4wOTQtMS40NjdjLTAuMDE4LTAuNTIxLTAuMDM2LTEuMDQxLTAuMDU2LTEuNTYxCgljLTAuMDI2LTAuNTk2LTAuMDI2LTEuMTkzLTAuMDU2LTEuNzg5Yy0wLjAyNC0wLjQ3Mi0wLjM3Ni0wLjg2OS0wLjg2OS0wLjg2OWMtMC4wNDUsMC0wLjA4NywwLjAyLTAuMTMxLDAuMDI3CgljLTAuMTMzLTAuMDk2LTAuMjg1LTAuMTY0LTAuNDU1LTAuMTVjLTAuMzksMC4wMzItMC43ODMsMC4wMjItMS4xNzUsMC4wMmMtMC40MDItMC4wMDItMC44MDUsMC4wMDQtMS4yMDUsMC4wMQoJYy0wLjc2LDAuMDE2LTEuNTIxLDAuMDEtMi4yNzktMC4wMWMtMC43NTctMC4wMi0xLjUxMS0wLjA3OC0yLjI2OC0wLjA5MmMtMC4xNjgtMC4wMDMtMC4zMzUtMC4wMDQtMC41MDMtMC4wMDQKCWMtMC42MiwwLTEuMjM3LDAuMDE3LTEuODU3LDAuMDI4Yy0wLjMyMiwwLjAwNi0wLjY0NiwwLjAwNi0wLjk2OSwwLjAwNmMtMC40MTQsMC0wLjgyOSwwLTEuMjQzLDAuMDEKCWMtMC4yNSwwLjAwOC0wLjQ3LDAuMTI1LTAuNjI0LDAuMzAyQzkuNDg1LDMuNzg0LDkuMzc1LDQsOS4zNjgsNC4yNDNDOS4zNTIsNC44NDksOS4zMzYsNS40NTYsOS4zMiw2LjA2MgoJQzkuMzA0LDYuNjA2LDkuMzEyLDcuMTQ5LDkuMzEyLDcuNjkxYzAsMC41NDQtMC4wMTQsMS4wODctMC4wMiwxLjYzMWMtMC4wMDUsMC40NDUsMC4wMTMsMC44OTYtMC4wMjUsMS4zNAoJYy0wLjA3NiwwLjM4OCwwLjA4OCwwLjc3LDAuNDI0LDAuOTZjMC4wMTIsMC4wMTQsMC4wMTcsMC4wMzMsMC4wMzEsMC4wNDZjMC4xNSwwLjE1LDAuMzI4LDAuMjA2LDAuNTM0LDAuMjIyCgljMC41MTYsMC4wNCwxLjAzNSwwLjAzMiwxLjU1MSwwLjAyNmMwLjI4NC0wLjAwNCwwLjU2Ni0wLjAwOCwwLjg0OS0wLjAwNmMwLjgwOSwwLjAxLDEuNjE2LTAuMDE5LDIuNDIzLTAuMDM1CgljMC4wMzUsMC41OTQsMC4wODIsMS4xODgsMC4xMiwxLjc4MmMtMC4wMDMsMC4wMDMtMC4wMDcsMC4wMDQtMC4wMDksMC4wMDdjLTAuMjQ4LDAuMjktMC40OSwwLjU4Ni0wLjc0OCwwLjg2OQoJYy0wLjI1NCwwLjI3OC0wLjUzLDAuNTM2LTAuNzg5LDAuODEzYy0wLjQ3NiwwLjUxLTAuOTE2LDEuMDU1LTEuNDExLDEuNTQ3Yy0wLjAzLDAtMC4wNTktMC4wMDMtMC4wODktMC4wMDMKCWMtMC4yMzIsMC0wLjQ2NCwwLjAxLTAuNjk5LDAuMDE5Yy0wLjQ1NCwwLjAxOC0wLjkwOSwwLjAyNi0xLjM2MywwLjAyOGMtMC4zMDQsMC4wMDItMC42MDgtMC4wMDItMC45MTEtMC4wMDgKCWMtMC42MDQtMC4wMTItMS4yMTEtMC4wMjItMS44MTUsMC4wMjZjLTAuMTMyLDAuMDEtMC4yNSwwLjA0My0wLjM2MSwwLjA5NGMtMC4wMDEsMC0wLjAwMSwwLTAuMDAxLDAKCWMtMC40NjUsMC0wLjg2MSwwLjM4OC0wLjg1NywwLjg1NGMwLjAwNCwwLjQ4MiwwLjAyNCwwLjk2MSwwLjA1OCwxLjQzOWMwLjAzMiwwLjQ0NiwwLjA4LDAuODg5LDAuMDgyLDEuMzM3CgljMC4wMDEsMC4xNDksMC4wNTMsMC4yODIsMC4xMjQsMC40MDRjLTAuMjI0LDAtMC40NDksMC4wMDEtMC42NzMsMC4wMDFjLTEuMDAxLTAuMDAyLTIuMDAxLTAuMDA0LTMsMC4wMjIKCWMtMC4yNjYsMC4wMDgtMC41MywwLjAyNC0wLjc5NywwLjA0OGMtMC4xNDgsMC4wMTMtMC4yNzcsMC4wNTctMC4zOTgsMC4xMjRDMS4yMjgsMjEuNDAxLDAuOTk3LDIxLjcwOCwxLDIyLjA2MgoJYzAuMDA2LDAuOTQ3LDAuMDMsMS44OTEsMC4wNzYsMi44MzZjMC4wMjIsMC40NCwwLjAyOCwwLjg3OSwwLjA0NiwxLjMxN2MwLjAyLDAuNDQ2LDAuMDM2LDAuODg5LDAuMDE2LDEuMzM1CgljLTAuMDEyLDAuMjksMC4xNjEsMC41MzksMC4zOTcsMC42ODljMC4xMzcsMC4yOCwwLjQxNSwwLjUsMC43MzYsMC40NzVjMC44OTctMC4wNzIsMS43OTMtMC4xNTIsMi42OTQtMC4xNwoJYzAuODA5LTAuMDE4LDEuNjE3LTAuMDA4LDIuNDI4LTAuMDE0YzAuMzU0LTAuMDAyLDAuNzA4LDAuMDAyLDEuMDY1LDAuMDA0YzAuMzgyLDAuMDA0LDAuNzY0LDAuMDA2LDEuMTQ3LDAuMDAyCgljMC4xOTYsMCwwLjM5NCwwLDAuNTksMGMwLjE5OCwwLjAwMiwwLjM5NiwwLjAwMiwwLjU5NCwwYzAuMzktMC4wMDIsMC43ODUsMC4wMDIsMS4xNzUsMC4wM2MwLjIxNywwLjAxNSwwLjQxNS0wLjA4MiwwLjU2MS0wLjIzMwoJYzAuMTcyLTAuMDI4LDAuMzM1LTAuMDk0LDAuNDU5LTAuMjE4YzAuMTA0LTAuMTA0LDAuMTc2LTAuMjI4LDAuMjE0LTAuMzY4YzAuMDM3LTAuMTM5LDAuMDQtMC4yNzcsMC4wMDctMC40MTYKCWMtMC4wMjctMC4zMTctMC4wMTYtMC42NC0wLjAxOS0wLjk1N2MtMC4wMDItMC40MjgtMC4wMS0wLjg1OS0wLjAxLTEuMjg3YzAtMS4wMTktMC4wMDgtMi4wMzUtMC4wMzYtMy4wNTIKCWMtMC4wMTItMC40NjYtMC4zOC0wLjg1Ny0wLjg1NS0wLjg1N2MtMC4wMTksMC0wLjAzNiwwLjAxLTAuMDU1LDAuMDEyYy0wLjExNy0wLjA1OS0wLjI0Mi0wLjA5OS0wLjM4NC0wLjExMQoJYy0wLjA3NC0wLjAwNi0wLjE0Ny0wLjAwOC0wLjIyMS0wLjAwOGMtMC4wOTIsMC0wLjE4NSwwLjAwMy0wLjI3NywwLjAwNGMtMC4xNzYsMC0wLjM1Mi0wLjAwNi0wLjUyOC0wLjAwMgoJYy0wLjM2OCwwLjAwOC0wLjc0LDAuMDEyLTEuMTExLDAuMDFjLTAuNjMtMC4wMDItMS4yNTktMC4wMDMtMS44ODktMC4wMDJjMC4wNzMtMC4xMjIsMC4xMjUtMC4yNTUsMC4xMTYtMC40MDMKCWMtMC4wNDEtMC42OTgtMC4wNTEtMS4zOTQtMC4wNTktMi4wOTFjMC4zMDItMC4wMjQsMC42MDUtMC4wNTIsMC45MDgtMC4wNjRjMC40MTgtMC4wMTgsMC44MzUtMC4wMTIsMS4yNTMtMC4wMjQKCWMwLjQ2NC0wLjAxMiwwLjkyOS0wLjA0MiwxLjM5My0wLjA2NGMwLjE5My0wLjAwOCwwLjM4NSwwLjAwOSwwLjU3NiwwLjAxM2MwLjI5NiwwLjMwNSwwLjU5NiwwLjYwNiwwLjg5NiwwLjkwOAoJYzAuMjg2LDAuMjksMC41ODIsMC41NzIsMC44NjUsMC44NjdjMC4yNjQsMC4yNzYsMC41MTIsMC41NjQsMC43ODIsMC44MzRjMC4yOSwwLjI5MiwwLjU3OCwwLjU4OCwwLjg3NSwwLjg3NwoJYzAuMTAyLDAuMDk5LDAuMjMsMC4xNTEsMC4zNjEsMC4xOGMwLjE1NCwwLjE0MSwwLjM1NiwwLjIyNiwwLjU2NSwwLjIyNmMwLjIsMCwwLjQ2NC0wLjA4LDAuNTkyLTAuMjQ0CgljMC4zNC0wLjQzNCwwLjY4Ni0wLjg1NywxLjA1MS0xLjI3MWMwLjI5LTAuMzI4LDAuNjE0LTAuNjE4LDAuOTE1LTAuOTM3YzAuMjYyLTAuMjc4LDAuNDg2LTAuNTg0LDAuNzQtMC44NjYKCWMwLjE3NC0wLjE5MywwLjM2My0wLjM3MiwwLjUzMi0wLjU2OWMwLjQ0NS0wLjAwOCwwLjg4OS0wLjAzOCwxLjMzNS0wLjA0N2MwLjQ5OC0wLjAxLDAuOTk5LDAsMS40OTksMC4wMDIKCWMwLjUxMiwwLjAwNCwxLjAyNy0wLjAwMiwxLjUzOS0wLjAxMmMwLjA2MS0wLjAwMSwwLjEyMy0wLjAwMSwwLjE4NS0wLjAwMWMwLjAwMSwwLjE5OCwwLjAwNCwwLjM5NywwLjAwOSwwLjU5NgoJYzAuMDEsMC4zODQsMC4wMiwwLjc2OSwwLjA1MiwxLjE1M2MwLjAwMSwwLjAwOCwwLjAwNCwwLjAxNCwwLjAwNSwwLjAyMmMtMC4wMDYsMC4zMjQtMC4wMDgsMC42NDktMC4wMDQsMC45NzUKCWMtMC40MTctMC4wMDktMC44MzUtMC4wMjMtMS4yNTMtMC4wMjNjLTAuMjA5LDAtMC40MTcsMC4wMDQtMC42MjYsMC4wMTRjLTAuNzUsMC4wMzYtMS41MDUsMC4wNDQtMi4yNTcsMC4wOQoJYy0wLjA4NiwwLjAwNi0wLjE3MiwwLjAxLTAuMjYsMC4wMThjLTAuMjIsMC4wMTYtMC40MSwwLjA3Ni0wLjU3LDAuMjM2Yy0wLjAwNSwwLjAwNS0wLjAwNiwwLjAxMS0wLjAxMSwwLjAxNgoJYy0wLjE4OSwwLjE1My0wLjMxOCwwLjM3Ny0wLjMyMSwwLjYzNWMtMC4wMDgsMC44NjksMC4wMDYsMS43MzUsMCwyLjYwNGMtMC4wMDQsMC40MjQsMC4wMDgsMC44NDksMC4wMDgsMS4yNzMKCWMwLDAuNDEtMC4wMDIsMC44MjEsMC4wMDgsMS4yMzFjMC4wMDUsMC4yMjgsMC4wOTksMC40MzgsMC4yNSwwLjU5YzAuMDc1LDAuMzc1LDAuNDAyLDAuNzE1LDAuODA1LDAuNjg3CgljMC44NTktMC4wNTgsMS43MTctMC4xMTYsMi41NzgtMC4xMjhjMC43ODEtMC4wMTIsMS41NTktMC4wNTQsMi4zNC0wLjA3NmMwLjc4NC0wLjAyNCwxLjU2OS0wLjAyLDIuMzU0LDAuMDEyCgljMC4zNjgsMC4wMTQsMC43MzYsMC4wMzgsMS4xMDUsMC4wNDRjMC4zMTgsMC4wMDUsMC42NDItMC4wMDUsMC45NjIsMC4wMmMwLjIxNSwwLjA0LDAuNDE3LDAuMDI3LDAuNjExLTAuMDg2CgljMC4xODItMC4xMDgsMC4zMi0wLjI4NiwwLjM3Ni0wLjQ5YzAuMDM0LTAuMTI1LDAuMDI4LTAuMjU1LDAuMDAyLTAuMzgxYzAuMDQ5LTAuMTA3LDAuMDgxLTAuMjIxLDAuMDkyLTAuMzQ3CgljMC4wMi0wLjI4OCwwLTAuNTg0LTAuMDA2LTAuODczQzMwLjk3MywyNS43ODMsMzAuOTU3LDI1LjM1OSwzMC45NDMsMjQuOTM0eiBNMTMuMTk4LDI3LjMwN2MwLTAuMDAyLTAuMDAxLTAuMDAzLTAuMDAxLTAuMDA1CgljLTAuMDAxLTAuMDA5LTAuMDAyLTAuMDE2LTAuMDAzLTAuMDI1QzEzLjE5NiwyNy4yODcsMTMuMTk3LDI3LjI5NywxMy4xOTgsMjcuMzA3eiBNMTEuMTk4LDIyLjc0CgljMC4wODUtMC4wMDEsMC4xNzIsMC4wMDEsMC4yNTgsMC4wMDJjMC4wMzMsMC43ODEsMC4wNzIsMS41NjIsMC4wOTIsMi4zNDVjMC4wMTIsMC40MjgtMC4wMDQsMC44NTgsMCwxLjI4NwoJYzAuMDAxLDAuMTk4LDAuMDEsMC4zOTcsMC4wMTUsMC41OTdjLTAuMDgsMC4wMDEtMC4xNi0wLjAwMi0wLjI0MSwwLjAwMWMtMC4xNiwwLjAwNC0wLjMyLDAuMDA4LTAuNDc4LDAuMDA0CgljLTAuMzk0LTAuMDA2LTAuNzg5LTAuMDA0LTEuMTgzLTAuMDA4Yy0wLjM3OC0wLjAwNC0wLjc1NiwwLTEuMTM1LDAuMDAyYy0wLjM3OCwwLjAwNC0wLjc1NywwLjAwNi0xLjEzNSwwLjAwMgoJYy0wLjU3OC0wLjAwNS0xLjE1Ni0wLjAxLTEuNzM0LTAuMDFjLTAuOTU2LDAtMS45MTEsMC4wMjEtMi44NjUsMC4wNzJjLTAuMDI0LTAuNzEyLTAuMDY1LTEuNDIzLTAuMDk4LTIuMTM1CgljLTAuMDM0LTAuNzI3LTAuMDIxLTEuNDU1LTAuMDA5LTIuMTgzYzAuNTYzLTAuMDM2LDEuMTI1LTAuMDU3LDEuNjkyLTAuMDU2YzAuNzcsMC4wMDQsMS41NDEsMC4wMTIsMi4zMTEsMC4wMTYKCUM4LjE5MiwyMi42ODYsOS42OTUsMjIuNzU2LDExLjE5OCwyMi43NHogTTEyLjQ0MiwxMC40MDdjLTAuNDg2LDAuMDEyLTAuOTczLDAuMDEzLTEuNDU5LDAuMDAxCgljLTAuMDAzLTAuMDcxLTAuMDA3LTAuMTQyLTAuMDA3LTAuMjExYzAtMC4yNjIsMC4wMDYtMC41MjQsMC4wMS0wLjc4N2MwLjAxMi0wLjU2LDAuMDEyLTEuMTE5LDAuMDA2LTEuNjc3CgljLTAuMDA2LTAuNTQ0LTAuMDMyLTEuMDg3LTAuMDMtMS42MzFjMC4wMDEtMC4zNzgsMC4wMTktMC43NTUsMC4wMzktMS4xMzNjMC42MjItMC4wMjYsMS4yNDUtMC4wNTIsMS44NjYtMC4wNzIKCWMwLjc2LTAuMDI2LDEuNTE1LTAuMDE0LDIuMjc1LDAuMDFjMC43NTksMC4wMjQsMS41MTUsMC4wNSwyLjI3MywwLjA3MmMwLjc3NiwwLjAyMiwxLjU1MywwLjAyOCwyLjMzLDAuMDM0CgljMC4zNCwwLjAwMiwwLjY3OC0wLjAwMiwxLjAxOS0wLjAwNmMwLjI1Ny0wLjAwMywwLjUxNC0wLjAwNSwwLjc3MiwwLjAwNmMwLjA2LDAuODk2LDAuMTE4LDEuNzkxLDAuMTQ4LDIuNjg4CgljMC4wMTYsMC40OSwwLjA2NCwwLjk3NywwLjA4OCwxLjQ2N2MwLjAxNiwwLjMzOSwwLjAzLDAuNjc5LDAuMDQ0LDEuMDE5Yy0wLjEyMiwwLjAwMy0wLjI0MywwLjAxLTAuMzY1LDAuMDEKCWMtMC4xMywwLTAuMjYsMC0wLjM4OCwwLjAwMmMtMC40MDIsMC4wMS0wLjgwNSwwLjAyMi0xLjIwOSwwLjAyOGMtMC44MDMsMC4wMS0xLjYwNSwwLjAzLTIuNDEsMC4wMzIKCWMtMC44MzUsMC0xLjY3MywwLjAwNC0yLjUwNiwwLjA0NEMxNC4xMDcsMTAuMzQ1LDEzLjI3NCwxMC4zODksMTIuNDQyLDEwLjQwN3ogTTE3Ljc2LDE4LjczOWMtMC4yODIsMC4zMDQtMC41ODYsMC41ODgtMC44NzMsMC44ODkKCWMtMC4yNzYsMC4yOS0wLjUzMywwLjU5NS0wLjc5MiwwLjg5OGMtMC4xNi0wLjE1Ny0wLjMxOS0wLjMxNS0wLjQ4LTAuNDdjLTAuMjcyLTAuMjYyLTAuNTI2LTAuNTQ0LTAuNzkxLTAuODE3CgljLTAuMjktMC4zLTAuNi0wLjU4Mi0wLjg4Ny0wLjg4NmMtMC4xMzMtMC4xNC0wLjI2NS0wLjI4My0wLjM5Ny0wLjQyNWMwLjE4Ni0wLjIyLDAuMzg2LTAuNDI4LDAuNTcxLTAuNjQ4CgljMC4yMjItMC4yNjgsMC40NDYtMC41MzQsMC42OC0wLjc5NWMwLjIzOC0wLjI2LDAuNDk2LTAuNTAyLDAuNzQyLTAuNzU0YzAuMTk1LTAuMjAxLDAuMzgxLTAuNDA5LDAuNTY4LTAuNjE2CgljMC40OSwwLjQzNywwLjk3OCwwLjg3NiwxLjQ3MywxLjMwOGMwLjQxNCwwLjM2MywwLjc4OCwwLjc3MywxLjE1OSwxLjE4M2MtMC4wNzYsMC4wNzktMC4xNDksMC4xNi0wLjIyMiwwLjI0MgoJQzE4LjI1NCwxOC4xMzcsMTguMDI2LDE4LjQ1NSwxNy43NiwxOC43Mzl6IE0yNy45OTksMjYuNzYxYy0wLjM5Ni0wLjAyLTAuNzkxLTAuMDQtMS4xODUtMC4wNDQKCWMtMC4wNTUtMC4wMDEtMC4xMDktMC4wMDEtMC4xNjQtMC4wMDFjLTAuMzQ1LDAtMC42ODksMC4wMTUtMS4wMzMsMC4wMjVjLTAuODAyLDAuMDI0LTEuNjA1LDAuMDU0LTIuNDEsMC4wNTYKCWMtMC42NzcsMC4wMDItMS4zNTIsMC4wMDUtMi4wMjcsMC4wMjZjLTAuMDE1LTAuNjk3LDAtMS4zOTQtMC4wMzUtMi4wOTFjLTAuMDMyLTAuNjQxLTAuMDIzLTEuMjc5LTAuMDA2LTEuOTIKCWMwLjUzNi0wLjAzNiwxLjA3Mi0wLjA3NCwxLjYwOS0wLjA5NWMwLjM3Mi0wLjAxNCwwLjc0Ni0wLjAwMiwxLjExOSwwLjAwNmMwLjM3NCwwLjAwOCwwLjc1LDAuMDA0LDEuMTI1LDAuMDA0CgljMC43NzktMC4wMDQsMS41NTUtMC4wMDYsMi4zMzIsMGMwLjMwOCwwLjAwNCwwLjYxNiwwLjAwMiwwLjkyNywwYzAuMzY2LTAuMDAyLDAuNzM0LTAuMDAxLDEuMTAyLDAuMDA0CgljLTAuMDIzLDAuNzM0LTAuMDI4LDEuNDY5LTAuMDA0LDIuMjA0YzAuMDE0LDAuNDEsMC4wMiwwLjgyMywwLjAxOCwxLjIzM2MwLDAuMjA5LDAuMDA3LDAuNDE5LDAuMDE0LDAuNjI4CglDMjguOTE4LDI2Ljc5OSwyOC40NTUsMjYuNzg1LDI3Ljk5OSwyNi43NjF6Ii8+Cjwvc3ZnPg==" />
    <script type="module">
      import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';

      // 初始化 Mermaid
      mermaid.initialize({ 
        startOnLoad: false,
        theme: 'default',
        // 禁止使用 iframe 渲染，解决 file:// 协议报错
        securityLevel: 'loose',
        // 可选：尝试减小 flowchart 的默认间距
        flowchart: {
          useMaxWidth: false, 
          htmlLabels: true
        }
      });

      // 手动渲染以便控制流程
      await mermaid.run({
        querySelector: '.mermaid'
      });

      // 渲染完成后执行初始化逻辑
      initZoom();
    </script>

    <!-- 2. 引入 svg-pan-zoom 库 -->
    <script src="https://cdn.jsdelivr.net/npm/svg-pan-zoom@3.6.1/dist/svg-pan-zoom.min.js"></script>

    <script>
      let panZoomInstance = null;
      let isPanning = false;
      let startX, startY;
      let startPanX, startPanY;

      function initZoom() {
        const svgElement = document.querySelector('.mermaid svg');
        const container = document.getElementById('zoomContainer');

        if (!svgElement) {
          console.error('未找到 SVG 元素');
          return;
        }

        // 修复 viewBox
        if (!svgElement.getAttribute('viewBox')) {
          const bbox = svgElement.getBoundingClientRect();
          if (bbox.width > 0 && bbox.height > 0) {
            svgElement.setAttribute('viewBox', `0 0 ${bbox.width} ${bbox.height}`);
          }
        }

        svgElement.setAttribute('width', '100%');
        svgElement.setAttribute('height', '100%');

        setTimeout(() => {
          try {
            // 初始化时禁用默认拖拽（panEnabled: false）
            panZoomInstance = svgPanZoom(svgElement, {
              zoomEnabled: true,
              controlIconsEnabled: false,
              fit: true,
              center: true,
              minZoom: 0.1,
              maxZoom: 10,
              zoomScaleSensitivity: 0.2,
              panEnabled: false, // 禁用默认拖拽，我们手动实现右键拖拽
              mouseWheelZoomEnabled: true,
              doubleClickZoomEnabled: true,
              preventMouseEventsDefault: false
            });

            // 绑定自定义右键拖拽逻辑
            setupRightClickPan(container, svgElement);
            
            // 绑定按钮事件
            setupButtons();

            console.log('svg-pan-zoom 初始化成功');
          } catch (error) {
            console.error('svg-pan-zoom 初始化失败:', error);
          }
        }, 100);
      }

      // 自定义右键拖拽逻辑
      function setupRightClickPan(container, svgElement) {
        // 禁用右键菜单
        svgElement.addEventListener('contextmenu', (e) => {
          e.preventDefault();
        });

        // 鼠标按下
        svgElement.addEventListener('mousedown', (e) => {
          // 只响应右键（button === 2）
          if (e.button === 2) {
            e.preventDefault();
            isPanning = true;
            startX = e.clientX;
            startY = e.clientY;
            
            // 获取当前平移位置
            const state = panZoomInstance.getPan();
            startPanX = state.x;
            startPanY = state.y;
            
            container.classList.add('panning');
          }
        });

        // 鼠标移动
        window.addEventListener('mousemove', (e) => {
          if (!isPanning) return;
          
          const deltaX = e.clientX - startX;
          const deltaY = e.clientY - startY;
          
          // 更新平移位置
          panZoomInstance.pan({
            x: startPanX + deltaX,
            y: startPanY + deltaY
          });
        });

        // 鼠标释放
        window.addEventListener('mouseup', () => {
          if (isPanning) {
            isPanning = false;
            container.classList.remove('panning');
          }
        });

        // 鼠标离开窗口
        window.addEventListener('mouseleave', () => {
          if (isPanning) {
            isPanning = false;
            container.classList.remove('panning');
          }
        });
      }

      // 按钮事件绑定
      function setupButtons() {
        // 居中按钮
        document.getElementById('centerBtn').addEventListener('click', () => {
          if (panZoomInstance) {
            panZoomInstance.center();
          }
        });

        // 重置按钮（居中 + 缩放比例恢复）
        document.getElementById('resetBtn').addEventListener('click', () => {
          if (panZoomInstance) {
            panZoomInstance.reset();
            panZoomInstance.center();
          }
        });

        // 放大按钮
        document.getElementById('zoomInBtn').addEventListener('click', () => {
          if (panZoomInstance) {
            panZoomInstance.zoomIn();
          }
        });

        // 缩小按钮
        document.getElementById('zoomOutBtn').addEventListener('click', () => {
          if (panZoomInstance) {
            panZoomInstance.zoomOut();
          }
        });
      }
    </script>
    <style>
      body {
        font-family: sans-serif;
        margin: 0;
        padding: 20px;
        background-color: #f4f4f4;
      }

      .controls {
        margin-bottom: 10px;
        display: flex;
        gap: 10px;
        align-items: center;
        flex-wrap: wrap;
      }

      .controls button {
        padding: 8px 16px;
        background-color: #6495ED;
        color: white;
        border: none;
        border-radius: 4px;
        cursor: pointer;
        font-size: 14px;
      }

      .controls button:hover {
        background-color: #007bff;
      }

      .controls span {
        color: #666;
        font-size: 13px;
        margin-left: 10px;
      }

      /* 缩放容器 */
      .zoom-container {
        width: 100%;
        height: 90vh;
        background-color: white;
        border: 1px solid #ccc;
        border-radius: 8px;
        overflow: hidden;
        display: flex;
        justify-content: center;
        align-items: center;
        box-shadow: 0 4px 6px rgba(0,0,0,0.1);
        position: relative;
      }

      /* Mermaid 原始样式保留 */
      .mermaid {
        font-family: 'Courier New', Courier, monospace !important;
        width: 100%;
        height: 100%;
        display: flex;
        justify-content: center;
      }
      
      /* 确保生成的 SVG 填满容器 */
      .mermaid svg {
        width: 100%;
        height: 100%;
        cursor: default; /* 默认光标，方便选择文本 */
        user-select: text;         /* 允许选择文本 */
        -webkit-user-select: text; /* Safari 兼容 */
        -moz-user-select: text;    /* Firefox 兼容 */
        -ms-user-select: text;     /* IE 兼容 */
      }

      /* SVG 内部文本元素也要允许选择 */
      .mermaid svg text {
        user-select: text !important;
        -webkit-user-select: text !important;
        cursor: text;
      }

      /* 拖拽时的光标 */
      .zoom-container.panning {
        cursor: grab;
      }

      .zoom-container.panning:active {
        cursor: grabbing;
      }

      /* 右键菜单禁用提示 */
      .zoom-container {
        -webkit-touch-callout: none;
      }
    </style>
  </head>
  <body>
    <!-- 控制按钮区域 -->
    <div class="controls">
      <button id="centerBtn">🎯 居中显示</button>
      <button id="resetBtn">🔄 重置视图</button>
      <button id="zoomInBtn">🔍 放大</button>
      <button id="zoomOutBtn">🔎 缩小</button>
      <span>💡 提示：右键拖拽移动 | 滚轮缩放 | 左键选择文本</span>
    </div>

    <div class="zoom-container" id="zoomContainer">
    <pre class="mermaid">
"###;

/// 准备 mermaid 的 html 内容
fn mermaid_html(content: &str) -> String {
    format!("{}{}
    </pre>
    </div>
  </body>
</html>", HTML, content)
}
