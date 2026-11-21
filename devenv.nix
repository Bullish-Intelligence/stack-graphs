{ pkgs, ... }:
{
  packages = [ pkgs.git pkgs.sccache ];
  languages.rust.enable = true;

  # Cache compiler output across worktrees and target directories.  Registry
  # crates and the tree-sitter grammars then build once per machine.
  #
  # Select a target directory with `cargo --target-dir`, not with the variable
  # CARGO_TARGET_DIR.  sccache hashes the CARGO_* variables, so a different
  # CARGO_TARGET_DIR makes every Rust crate miss the cache.
  env.RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
}
