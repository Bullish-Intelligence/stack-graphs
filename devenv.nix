{ pkgs, lib, ... }:
let
  # One sccache server for each sccache build.  A client and a server of different
  # versions cannot talk: the client fails with "invalid value: integer `13`".  A
  # worktree without devenv.lock takes a newer nixpkgs, so it has a newer sccache.
  # The default port 4226 is shared, so the first server to start would break the others.
  # The port comes from the store path of sccache, in 20000 to 27999.  That range is
  # below the ephemeral ports and above the hashed port range of Scippy.
  sccachePort = 20000 + lib.mod (lib.fromHexString (builtins.substring 0 4 (builtins.hashString "sha256" "${pkgs.sccache}"))) 8000;
in
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
  env.SCCACHE_SERVER_PORT = toString sccachePort;
}
