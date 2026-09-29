// The extension's one job beyond highlighting: run the patch you are looking
// at. Hot reload does the rest — run once, then every save swaps the show.
const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

let terminal = null;

// The command prefix `run <file>` is appended to. Explicit setting wins;
// inside the engine repo the CLI runs from source; elsewhere `vybe` is on PATH.
function runner(folder) {
  const configured = vscode.workspace.getConfiguration('vybe').get('command');
  if (configured && configured !== 'vybe') return configured;
  if (folder && fs.existsSync(path.join(folder, 'crates', 'vybe-cli'))) {
    return 'cargo run -p vybe-cli --';
  }
  return 'vybe';
}

function quoted(p) {
  return /\s/.test(p) ? `"${p}"` : p;
}

async function runPatch(uri) {
  const editor = vscode.window.activeTextEditor;
  const target = uri || (editor && editor.document.uri);
  if (!target || !target.fsPath.endsWith('.vy')) {
    vscode.window.showErrorMessage('vybe: no .vy patch to run.');
    return;
  }
  const doc = vscode.workspace.textDocuments.find(
    (d) => d.uri.toString() === target.toString()
  );
  if (doc && doc.isDirty) await doc.save(); // the player reads the file

  const ws = vscode.workspace.getWorkspaceFolder(target);
  const cwd = ws ? ws.uri.fsPath : path.dirname(target.fsPath);
  const file = ws ? path.relative(cwd, target.fsPath) : target.fsPath;

  if (terminal) terminal.dispose(); // one show at a time
  terminal = vscode.window.createTerminal({ name: 'vybe', cwd });
  terminal.show(true);
  terminal.sendText(`${runner(cwd)} run ${quoted(file)}`);
}

// rust-analyzer anchors its lens on `fn main`; a patch's main is its `out`
// line — absent that, the first line that says anything.
function anchorLine(document) {
  let firstContent = 0;
  let found = false;
  for (let i = 0; i < document.lineCount; i++) {
    const text = document.lineAt(i).text.trim();
    if (/^out\b/.test(text)) return i;
    if (!found && text && !text.startsWith('#')) {
      firstContent = i;
      found = true;
    }
  }
  return firstContent;
}

const lenses = {
  provideCodeLenses(document) {
    const line = anchorLine(document);
    return [
      new vscode.CodeLens(new vscode.Range(line, 0, line, 0), {
        title: '▶ Run',
        tooltip: 'vybe run — hot-reloads on save',
        command: 'vybe.runPatch',
        arguments: [document.uri],
      }),
    ];
  },
};

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('vybe.runPatch', runPatch),
    vscode.languages.registerCodeLensProvider({ language: 'vy' }, lenses),
    vscode.window.onDidCloseTerminal((t) => {
      if (t === terminal) terminal = null;
    })
  );
}

function deactivate() {
  if (terminal) terminal.dispose();
}

module.exports = { activate, deactivate };
