//! vybe-host — the daemon that boots into the show and never quits.
//!
//! Today it does one honest thing: scan the project registry and say what
//! a boot would play. Supervision (spawn the Player, restart on death, fall
//! back to the last good patch) and the control API land when the appliance
//! kata pulls them — see the README.

use std::fs;
use std::path::PathBuf;

/// A project is a folder with a `project.toml` — the unit the host can boot.
struct Project {
    name: String,
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/vybe/projects"));

    let mut projects: Vec<Project> = match fs::read_dir(&root) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("project.toml").exists())
            .map(|p| Project {
                name: p.file_name().unwrap().to_string_lossy().into_owned(),
            })
            .collect(),
        Err(e) => {
            eprintln!("vybe-host: cannot read {}: {e}", root.display());
            std::process::exit(1);
        }
    };
    projects.sort_by(|a, b| a.name.cmp(&b.name));

    // The default mark is a one-line file beside the projects, not a field
    // inside them: "set as startup" rewrites one word, atomically.
    let default = fs::read_to_string(root.join("default"))
        .map(|s| s.trim().to_string())
        .ok();

    println!(
        "vybe-host: {} project(s) in {}",
        projects.len(),
        root.display()
    );
    for p in &projects {
        let mark = if Some(&p.name) == default.as_ref() {
            " (default)"
        } else {
            ""
        };
        println!("  {}{mark}", p.name);
    }

    match default {
        Some(name) if projects.iter().any(|p| p.name == name) => {
            println!("boot would play: {name}");
        }
        Some(name) => {
            eprintln!("vybe-host: default project '{name}' not found — boot would fail");
            std::process::exit(1);
        }
        None => {
            eprintln!("vybe-host: no default project set — nothing to boot into");
            std::process::exit(1);
        }
    }
}
