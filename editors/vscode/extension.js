// The extension's one job beyond highlighting: run the patch you are looking
// at. Hot reload does the rest — run once, then every save swaps the show.
const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

// Run and check keep separate terminals: a check must never kill the show.
const terminals = { run: null, check: null };

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

async function inTerminal(kind, name, verb, uri) {
  const editor = vscode.window.activeTextEditor;
  const target = uri || (editor && editor.document.uri);
  if (!target || !target.fsPath.endsWith('.vy')) {
    vscode.window.showErrorMessage(`vybe: no .vy patch to ${verb}.`);
    return;
  }
  const doc = vscode.workspace.textDocuments.find(
    (d) => d.uri.toString() === target.toString()
  );
  if (doc && doc.isDirty) await doc.save(); // the CLI reads the file

  const ws = vscode.workspace.getWorkspaceFolder(target);
  const cwd = ws ? ws.uri.fsPath : path.dirname(target.fsPath);
  const file = ws ? path.relative(cwd, target.fsPath) : target.fsPath;

  if (terminals[kind]) terminals[kind].dispose(); // one show / one report
  terminals[kind] = vscode.window.createTerminal({ name, cwd });
  terminals[kind].show(true);
  terminals[kind].sendText(`${runner(cwd)} ${verb} ${quoted(file)}`);
}

const runPatch = (uri) => inTerminal('run', 'vybe', 'run', uri);
const checkPatch = (uri) => inTerminal('check', 'vybe check', 'check', uri);

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
    const at = new vscode.Range(anchorLine(document), 0, anchorLine(document), 0);
    return [
      new vscode.CodeLens(at, {
        title: '▶ Run',
        tooltip: 'vybe run — hot-reloads on save',
        command: 'vybe.runPatch',
        arguments: [document.uri],
      }),
      new vscode.CodeLens(at, {
        title: 'Check',
        tooltip: 'vybe check — prove the patch, no GPU; a running show stays up',
        command: 'vybe.checkPatch',
        arguments: [document.uri],
      }),
    ];
  },
};

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('vybe.runPatch', runPatch),
    vscode.commands.registerCommand('vybe.checkPatch', checkPatch),
    vscode.languages.registerCodeLensProvider({ language: 'vy' }, lenses),
    vscode.window.onDidCloseTerminal((t) => {
      for (const kind of Object.keys(terminals)) {
        if (terminals[kind] === t) terminals[kind] = null;
      }
    })
  );
}

function deactivate() {
  for (const kind of Object.keys(terminals)) {
    if (terminals[kind]) terminals[kind].dispose();
  }
}

module.exports = { activate, deactivate };
