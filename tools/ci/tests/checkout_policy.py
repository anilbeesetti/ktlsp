"""Parser-free Git byte-policy regression; not PowerShell or Windows execution proof.

Run with python3 tools/ci/tests/checkout_policy.py. Temporary fixture repositories
only: no Rust, language parser, JVM, workflow dispatch or existing worktree edits.
The historical Windows artifact did not retain its byte/config cause, so this
checks the explicit policy contract, not a reproduction of that original failure.
"""

import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]
POLICY = {
    "GIT_CONFIG_COUNT": "2",
    "GIT_CONFIG_KEY_0": "core.autocrlf",
    "GIT_CONFIG_VALUE_0": "false",
    "GIT_CONFIG_KEY_1": "core.eol",
    "GIT_CONFIG_VALUE_1": "lf",
}


def main():
    for name in ("native-platform-checks.yml", "native-packaging-experiment.yml"):
        source = (ROOT / ".github/workflows" / name).read_text()
        checkout = source.split("      - uses: actions/checkout@v4\n")
        assert len(checkout) == 2, name
        checkout = checkout[1].split("      - name:", 1)[0]
        for key, value in POLICY.items():
            assert f"          {key}: {value}\n" in checkout or (
                f"          {key}: '{value}'\n" in checkout
            ), (name, key)
        assert "          persist-credentials: false\n" in checkout, name
    helper = (ROOT / "tools/ci/native-source-preflight.ps1").read_text()
    assert "git config --local core.autocrlf false" in helper
    assert "git config --local core.eol lf" in helper
    assert "$limit = 65536\n" in helper
    assert "if ($head -cne $env:SOURCE_SHA -or $tree -cnotmatch '^[0-9a-f]{40}$' -or $status.Count -ne 0) {" in helper

    # Isolate Git configuration, including any environment-injected configuration.
    clean_env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    clean_env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
    checkout_env = clean_env | POLICY
    with tempfile.TemporaryDirectory(prefix="ktlsp-git-policy-") as temp:
        repo = Path(temp)
        template = repo / "empty-template"
        template.mkdir()

        def git(*args, policy=False):
            result = subprocess.run(
                ["git", *args], cwd=repo,
                env=checkout_env if policy else clean_env,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
            )
            return result.stdout

        git("init", "--quiet", f"--template={template}")
        # Simulate an inherited CRLF-oriented runner policy without changing
        # any real global configuration. First materialization must override it.
        git("config", "--local", "core.autocrlf", "true")
        git("config", "--local", "core.eol", "crlf")
        expected = {"text.rs": b"fn main() {}\n// LF source\n", "data.bin": b"\x00native\r\nfixture\n"}
        for name, data in expected.items():
            (repo / name).write_bytes(data)
        git("add", "--", *expected, policy=True)
        git(
            "-c", "user.name=ktlsp CI fixture", "-c", "user.email=ci-fixture@invalid",
            "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Parser-free byte-policy fixture",
            policy=True,
        )
        indexed = git("ls-files", "--stage", "--eol")
        assert b"i/lf" in indexed and b"i/-text" in indexed
        for name in expected:
            (repo / name).unlink()
        git("checkout-index", "--all", "--force", policy=True)
        for name, data in expected.items():
            assert (repo / name).read_bytes() == data, name

        # Mirror the helper's repository-local persistence. Later default Git
        # commands, including the final packaging guard, use the same policy.
        git("config", "--local", "core.autocrlf", "false")
        git("config", "--local", "core.eol", "lf")
        assert git("config", "--get", "core.autocrlf") == b"false\n"
        assert git("config", "--get", "core.eol") == b"lf\n"
        git("-c", "core.autocrlf=false", "-c", "core.eol=lf", "checkout-index", "--all", "--force")
        assert git("status", "--porcelain=v1", "--untracked-files=all") == b""
        raw_ids = git("hash-object", "--no-filters", "--", *expected).splitlines()
        indexed_ids = {
            line.split(b"\t", 1)[1].decode(): line.split()[1]
            for line in git("ls-files", "--stage").splitlines()
        }
        assert raw_ids == [indexed_ids[name] for name in expected]
        (repo / "text.rs").write_bytes(expected["text.rs"] + b"// real edit\n")
        assert b" M text.rs" in git("status", "--porcelain=v1", "--untracked-files=all")
        (repo / "text.rs").write_bytes(expected["text.rs"])
        (repo / "unexpected.txt").write_bytes(b"untracked\n")
        assert b"?? unexpected.txt" in git("status", "--porcelain=v1", "--untracked-files=all")
    print("PASS: fixed first-checkout policy, exact fixture bytes, persistent clean guard, tracked/untracked dirt rejection")


if __name__ == "__main__":
    main()
