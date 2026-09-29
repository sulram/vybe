# vybe patches (.vy) — highlighting + run

Highlighting for vybe's `.vy` patches in VS Code (and anything else that reads
TextMate grammars: Cursor, Zed, Sublime, GitHub), plus a run command the way
rust-analyzer runs an example.

## Run a patch

A `▶ Run` lens sits on the patch's `out` line — its `fn main` — or on the
first content line when no `out` is declared (also: the play button in the
editor title, or **vybe: Run Patch** in the palette). It saves the file and
runs it in a `vybe` terminal — run once, then every save hot-reloads into the
playing show; re-running replaces the terminal (one show at a time).

The command prefix comes from the `vybe.command` setting (default `vybe`,
from PATH). Inside the engine repo — `crates/vybe-cli` present — it becomes
`cargo run -p vybe-cli --` on its own, so this workspace needs no config.

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
