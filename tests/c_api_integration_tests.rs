use std::env;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn run_c_api_tests() {
    // Ensure the static library is built first by running `cargo build`
    let cargo_build_status = Command::new(env!("CARGO"))
        .arg("build")
        .status()
        .expect("Failed to execute cargo build");
    assert!(
        cargo_build_status.success(),
        "cargo build failed before C API test setup"
    );

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let header_dir = manifest_dir.join("header");
    let c_test_file = manifest_dir.join("tests").join("c_api_tests.c");

    let target_dir = manifest_dir.join("target");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let out_exe_dir = target_dir.join("c_api_tests_output");
    std::fs::create_dir_all(&out_exe_dir).expect("Failed to create C test output directory");

    let exe_name = if cfg!(windows) {
        "c_api_runner.exe"
    } else {
        "c_api_runner"
    };
    let exe_path = out_exe_dir.join(exe_name);

    // Compile the C test against the static library `libvi.a`
    // The static lib `libvi.a` should be in `target/{profile}/`
    let rust_lib_path = target_dir.join(profile);

    let mut cc_build = cc::Build::new();
    cc_build
        .file(&c_test_file)
        .include(&header_dir)
        .cargo_metadata(false) // Don't print cargo metadata from this cc build
        .out_dir(&out_exe_dir) // Control where the .o file goes
        .object("c_api_tests_obj"); // Name for the .o file

    // Add linker arguments for the Rust static library
    // The name of the static library is lib<crate_name>.a
    let crate_name = env!("CARGO_PKG_NAME");
    let lib_name = format!("lib{}.a", crate_name);
    let full_lib_path = rust_lib_path.join(&lib_name);

    if !full_lib_path.exists() {
        panic!(
            "Rust static library {} not found. Ensure it's built (e.g., via `cargo build`).",
            full_lib_path.display()
        );
    }

    // For cc::Build, to link a static library, you add its directory to search path
    // and then specify the library name without "lib" and ".a".
    // However, cc::Build `compile` method produces an object file, not an executable.
    // We need to invoke the linker manually or use `cc::Build::link` if that were a thing.
    // Simpler: compile and link in one step with Command.

    let mut compiler_builder = cc::Build::new();
    // Try to get TARGET from env, fallback to a common default if not found (though it should be there)
    let target_triple = std::env::var("TARGET").unwrap_or_else(|_| {
        // Attempt to infer a reasonable default, or use a hardcoded one.
        // For most local test runs, host triple is a good guess.
        // This could be made more robust (e.g. using `rustc -vV`) but let's keep it simple.
        if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
            "x86_64-unknown-linux-gnu".to_string()
        } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
            "x86_64-pc-windows-msvc".to_string() // or gnu
        } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
            "x86_64-apple-darwin".to_string()
        } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin".to_string()
        } else {
            eprintln!("Warning: TARGET env var not found, using a potentially incorrect default.");
            "x86_64-unknown-linux-gnu".to_string() // A common default
        }
    });

    compiler_builder.target(&target_triple);
    compiler_builder.host(&target_triple); // Assume host == target for this test scenario

    if cfg!(debug_assertions) {
        compiler_builder.opt_level(0).debug(true);
    } else {
        // Match release build settings if possible, though not strictly necessary for test execution
        compiler_builder.opt_level(3);
    }
    let compiler = compiler_builder.get_compiler(); // Gets the C compiler tool

    let mut command = Command::new(compiler.path());
    command
        .arg(c_test_file)
        .arg("-o")
        .arg(&exe_path)
        .arg(format!("-I{}", header_dir.display()))
        .arg(format!("-L{}", rust_lib_path.display()))
        .arg(format!("-l{}", crate_name)); // Link against `vi` (expects libvi.a or libvi.so)

    // Add platform-specific linker args if necessary (e.g. for pthreads, dl, etc. on Linux)
    if cfg!(target_os = "linux") {
        command.arg("-lpthread").arg("-ldl");
    }

    println!("Compiling C tests with command: {:?}", command);
    let status = command.status().expect("Failed to compile C tests");

    assert!(
        status.success(),
        "C test compilation failed. Compiler output: {:?}",
        status
    );

    // Run the compiled C test executable
    println!("Running C test executable: {:?}", exe_path);
    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run C test executable");

    println!(
        "C Test STDOUT:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    eprintln!(
        "C Test STDERR:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        output.status.success(),
        "C API tests failed (executable returned non-zero exit code). Check assertions in C code."
    );
}
