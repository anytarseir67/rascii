fn main() {
    std::process::Command::new("cargo").args(["pgo", "instrument", "run", "--", "-v", "--bin", "rascii", "--", "-i", "pgo.mp4", "-o", "pgo_out.mp4"]).spawn().unwrap().wait().unwrap();
    std::process::Command::new("cargo").args(["pgo", "optimize", "build"]).spawn().unwrap().wait().unwrap();
    std::fs::remove_file("./pgo_out.mp4").unwrap();
}
