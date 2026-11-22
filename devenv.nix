{ pkgs, lib, ... }:
let
  # One sccache server for each sccache build.  A client and a server of different
  # versions cannot talk: the client fails with "invalid value: integer `13`".  A
  # worktree without devenv.lock takes a newer nixpkgs, so it has a newer sccache.
  # The default port 4226 is shared, so the first server to start would break the others.
  # The port comes from the store path of sccache, in 20000 to 27999.  That range is
  # below the ephemeral ports and above the hashed port range of Scippy.
  portMin = 20000;
  portSpan = 8000;
  sccachePort = portMin + lib.mod (lib.fromHexString (builtins.substring 0 4 (builtins.hashString "sha256" "${pkgs.sccache}"))) portSpan;
  # sccache waits for ever when a program that is not sccache listens on its port.
  # This guard keeps the derived port when no program listens on it, or when an sccache server
  # answers on it. The port comes from the store path, so that server has the same version.
  # Otherwise the guard tries the next port in the range.
  # After 16 busy ports it unsets RUSTC_WRAPPER and warns, so the build runs without sccache.
  sccacheGuard = ''
    sccache_listens() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }
    sccache_ours() {
      SCCACHE_SERVER_PORT=$1 ${pkgs.coreutils}/bin/timeout 2 ${pkgs.sccache}/bin/sccache --show-stats >/dev/null 2>&1
    }
    sccache_guard() {
      [ -n "$RUSTC_WRAPPER" ] || return 0
      local start=$SCCACHE_SERVER_PORT i cand
      for i in $(seq 0 15); do
        cand=$(( ${toString portMin} + (start - ${toString portMin} + i) % ${toString portSpan} ))
        if ! sccache_listens "$cand" || sccache_ours "$cand"; then
          export SCCACHE_SERVER_PORT=$cand
          return 0
        fi
      done
      echo "warning: 16 sccache ports are busy from $start. Building without sccache." >&2
      unset RUSTC_WRAPPER
    }
    sccache_guard
  '';
in
{
  packages = [ pkgs.git pkgs.sccache ];
  languages.rust.enable = true;
  enterShell = sccacheGuard;

  # Cache compiler output across worktrees and target directories.  Registry
  # crates and the tree-sitter grammars then build once per machine.
  #
  # Select a target directory with `cargo --target-dir`, not with the variable
  # CARGO_TARGET_DIR.  sccache hashes the CARGO_* variables, so a different
  # CARGO_TARGET_DIR makes every Rust crate miss the cache.
  env.RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
  env.SCCACHE_SERVER_PORT = toString sccachePort;
}
