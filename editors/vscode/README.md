# vybe patches (.vy) — syntax highlighting

Highlighting for vybe's `.vy` patches in VS Code (and anything else that reads
TextMate grammars: Cursor, Zed, Sublime, GitHub).

## Install (from this repo)

VS Code loads any folder in its extensions directory — link this one in:

    ln -s "$PWD/editors/vscode" ~/.vscode/extensions/vybe-vy

then run **Developer: Reload Window**. To remove it, delete that link.

## The grammar is generated — don't edit it

`syntaxes/vy.tmLanguage.json` is printed by the engine from its own vocabulary
table, so highlighting can't fall behind the language:

    cargo run -p vybe-cli -- grammar > editors/vscode/syntaxes/vy.tmLanguage.json

A test fails when the checked-in file is stale. To change a *rule*, edit
`src/patch/vy.tmLanguage.template.json`; to add a *word*, add its row to
`src/patch/vocabulary.rs` — then regenerate.
