/**
 * Build script for Termin.AI
 *
 * This script bundles the TypeScript agent code using pnpm + vite
 * during the Rust build process. The bundled JS is then embedded
 * into the Rust binary using include_str!().
 */
use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
  // Tell cargo about our custom cfg
  println!("cargo::rustc-check-cfg=cfg(has_bundled_agent)");

  println!("cargo:rerun-if-changed=../typescript/agent/");
  println!("cargo:rerun-if-changed=../typescript/package.json");
  println!("cargo:rerun-if-changed=../typescript/vite.config.ts");
  println!("cargo:rerun-if-changed=../typescript/tsconfig.json");

  let manifest_dir =
    env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
  let typescript_dir = Path::new(&manifest_dir).join("../typescript");

  // Check if we're in release mode
  let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
  let is_release = profile == "release";

  // Install dependencies if node_modules doesn't exist
  let node_modules = typescript_dir.join("node_modules");
  if !node_modules.exists() {
    println!("cargo:warning=Installing TypeScript dependencies...");

    let status = Command::new("pnpm")
      .arg("install")
      .arg("--frozen-lockfile")
      .current_dir(&typescript_dir)
      .status();

    match status {
      Ok(s) if s.success() => {
        println!(
          "cargo:warning=TypeScript dependencies installed successfully"
        );
      }
      Ok(s) => {
        // Try npm as fallback
        println!(
          "cargo:warning=pnpm install failed ({}), trying npm...",
          s.code().unwrap_or(-1)
        );
        let npm_status = Command::new("npm")
          .arg("install")
          .current_dir(&typescript_dir)
          .status();

        match npm_status {
          Ok(s) if s.success() => {
            println!(
              "cargo:warning=TypeScript dependencies installed with npm"
            );
          }
          _ => {
            println!("cargo:warning=Failed to install TypeScript dependencies");
            println!(
              "cargo:warning=Please run 'pnpm install' in the typescript directory"
            );
            // Don't fail the build - we'll use the embedded fallback JS
          }
        }
      }
      Err(e) => {
        println!("cargo:warning=pnpm not found: {}", e);
        println!("cargo:warning=Trying npm...");
        let npm_status = Command::new("npm")
          .arg("install")
          .current_dir(&typescript_dir)
          .status();

        match npm_status {
          Ok(s) if s.success() => {
            println!(
              "cargo:warning=TypeScript dependencies installed with npm"
            );
          }
          _ => {
            println!("cargo:warning=Neither pnpm nor npm available");
            // Don't fail the build - we'll use the embedded fallback JS
          }
        }
      }
    }
  }

  // Build TypeScript
  let build_script = if is_release {
    "build:release"
  } else {
    "build:debug"
  };

  println!(
    "cargo:warning=Building TypeScript agent ({} mode)...",
    if is_release { "release" } else { "debug" }
  );

  let status = Command::new("pnpm")
    .arg("run")
    .arg(build_script)
    .current_dir(&typescript_dir)
    .status();

  let build_succeeded = match status {
    Ok(s) if s.success() => {
      println!("cargo:warning=TypeScript agent built successfully");
      true
    }
    Ok(s) => {
      println!(
        "cargo:warning=pnpm build failed ({}), trying npm...",
        s.code().unwrap_or(-1)
      );
      let npm_status = Command::new("npm")
        .arg("run")
        .arg(build_script)
        .current_dir(&typescript_dir)
        .status();

      match npm_status {
        Ok(s) if s.success() => {
          println!("cargo:warning=TypeScript agent built with npm");
          true
        }
        _ => {
          println!("cargo:warning=Failed to build TypeScript agent");
          false
        }
      }
    }
    Err(e) => {
      println!("cargo:warning=pnpm not found: {}", e);
      let npm_status = Command::new("npm")
        .arg("run")
        .arg(build_script)
        .current_dir(&typescript_dir)
        .status();

      match npm_status {
        Ok(s) if s.success() => {
          println!("cargo:warning=TypeScript agent built with npm");
          true
        }
        _ => {
          println!("cargo:warning=Failed to build TypeScript agent");
          false
        }
      }
    }
  };

  // Set environment variable to indicate whether we have a bundled agent
  let dist_file = typescript_dir.join("dist/agent.js");
  if build_succeeded && dist_file.exists() {
    println!(
      "cargo:rustc-env=TERMINAI_AGENT_JS_PATH={}",
      dist_file.display()
    );
    println!("cargo:rustc-cfg=has_bundled_agent");
  } else {
    println!(
      "cargo:warning=Using fallback embedded agent (TypeScript build not available)"
    );
  }
}
