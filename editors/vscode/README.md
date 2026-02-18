# Aion Language Support

VS Code support for the Aion programming language.

## Features

- **Syntax Highlighting**:
  - Keywords: `fn`, `let`, `struct`, `enum`, `intent`, `spawn`, `unsafe`, etc.
  - Types: `i64`, `f64`, `String`, etc.
  - Operators: `|>`, `<-`, `&&`, `||`, etc.
  - Literals: Strings, f-strings (`f"Hello {name}"`), Durations (`5s`, `10d`), Dates (`D2024-01-01`).
- **Snippets**: Quick templates for `fn`, `main`, `struct`, `enum`, `match`, `spawn`, and more.
- **Language Configuration**: Smart brackets, auto-closing pairs, and indentation rules.

## Installation

1. Copy the `editors/vscode` folder to your VS Code extensions directory:
   - Windows: `%USERPROFILE%\.vscode\extensions`
   - macOS/Linux: `~/.vscode/extensions`
2. Or, open the `editors/vscode` folder in VS Code and press `F5` to test it.

## Extension Settings

This extension contributes the following settings:

* `aion.enable`: Enable/disable this extension.

## Tasks Configuration

You can add the following to your `.vscode/tasks.json` to easily build and run Aion files:

```json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "Aion: Build current file",
            "type": "shell",
            "command": "./aion build ${file}",
            "group": {
                "kind": "build",
                "isDefault": true
            }
        },
        {
            "label": "Aion: Run current file",
            "type": "shell",
            "command": "./aion build ${file} && ./output",
            "group": "test",
            "presentation": {
                "reveal": "always",
                "panel": "new"
            }
        }
    ]
}
```
