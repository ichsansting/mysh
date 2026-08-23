Feature: Add starts tracking a new path in Source
  Add dispatches on what exists on disk under Target: a file is copied into
  Source (or encrypted there with --secret), a directory becomes a
  .track-marked mirror after confirmation, and a path that exists nowhere is
  treated as a package specifier (lazy by default). Add only ever touches
  Source — never Target, never git history.

  Scenario: file add copies untracked target content into source
    Given target file ".vimrc" already exists with content "set number"
    When I run "add .vimrc"
    Then it succeeds
    And source ".vimrc" contains exactly "set number"

  Scenario: file add leaves a subsequent diff showing no drift
    Given a bare remote
    And target file ".vimrc" already exists with content "set number"
    And I ran "add .vimrc"
    When I run "diff"
    Then the output reports no target drift

  Scenario: file add on an already tracked path errors without modifying source
    Given source file ".vimrc" with content "original"
    And target file ".vimrc" already exists with content "edited"
    When I run "add .vimrc"
    Then it fails mentioning "already tracked"
    And source ".vimrc" contains exactly "original"

  Scenario: file add with --secret writes an age-suffixed file that round trips
    Given target file ".netrc" already exists with content "machine x login y"
    When I run "add --secret .netrc"
    Then it succeeds
    And source ".netrc.age" exists
    And source ".netrc.age" does not contain the plaintext "machine x login y"
    And re-rendering ".netrc" from source yields exactly "machine x login y"

  Scenario: file add refuses a plain copy when the secret variant is already tracked
    Given source secret ".netrc.age" encrypting "machine x login y"
    And target file ".netrc" already exists with content "machine x login y"
    When I run "add .netrc"
    Then it fails mentioning "already tracked"
    And source ".netrc" does not exist

  Scenario: --secret combined with a directory errors and writes nothing
    Given target directory ".config/app" exists containing file "a.toml" with "x=1"
    When I run "add --secret .config/app"
    Then it fails
    And source ".config/app" does not exist

  Scenario: --secret combined with a package specifier errors and writes nothing
    When I run "add --secret ripgrep"
    Then it fails
    And source ".mysh/bin/ripgrep" does not exist

  Scenario: folder add confirmed creates the .track marker and copies matched files
    Given target directory ".config/app" exists containing file "a.toml" with "x=1"
    And target file ".config/app/b.toml" already exists with content "y=2"
    When I run "add .config/app" answering "y"
    Then it succeeds
    And source ".config/app/.track" exists
    And source ".config/app/a.toml" contains exactly "x=1"
    And source ".config/app/b.toml" contains exactly "y=2"

  Scenario: folder add declined leaves source completely unchanged
    Given target directory ".config/app" exists containing file "a.toml" with "x=1"
    And I record the state of the source tree
    When I run "add .config/app" answering "n"
    Then it succeeds
    And no file under the source changed

  Scenario: folder add declined on a nested path leaves no empty parent directories behind
    Given target directory ".config/deep/nested/app" exists containing file "a.toml" with "x=1"
    When I run "add .config/deep/nested/app" answering "n"
    Then source ".config" does not exist

  Scenario: folder add --ignore pattern excludes matching files from the copy
    Given target directory ".config/app" exists containing file "a.toml" with "x=1"
    And target file ".config/app/cache.log" already exists with content "noise"
    When I run "add --ignore *.log .config/app" answering "y"
    Then source ".config/app/a.toml" contains exactly "x=1"
    And source ".config/app/cache.log" does not exist
    And source ".config/app/.track" contains exactly "*.log"

  Scenario: folder add on an already tracked directory errors without modifying source
    Given source directory ".config/app" tracked with an empty ".track" marker
    And source file ".config/app/a.toml" with content "x=1"
    And target directory ".config/app" exists containing file "a.toml" with "x=1"
    When I run "add .config/app"
    Then it fails mentioning "already tracked"

  Scenario: package add defaults to lazy and writes a real portable shim file
    When I run "add ripgrep"
    Then it succeeds
    And source ".mysh/bin/ripgrep" exists
    And the shim ".mysh/bin/ripgrep" invokes mise with specifier "ripgrep"
    And the shim ".mysh/bin/ripgrep" contains no absolute device-specific path
    And the shim ".mysh/bin/ripgrep" is not marked eager

  Scenario: package add lazy honors the --bin override
    When I run "add --bin rg ripgrep"
    Then it succeeds
    And source ".mysh/bin/rg" exists
    And the shim ".mysh/bin/rg" invokes mise with specifier "ripgrep"

  Scenario: package add --eager writes the same shim with the eager marker
    When I run "add --eager fish"
    Then it succeeds
    And source ".mysh/bin/fish" exists
    And the shim ".mysh/bin/fish" invokes mise with specifier "fish"
    And the shim ".mysh/bin/fish" is marked eager

  Scenario: package add --eager honors the --bin override
    When I run "add --eager --bin nu nushell"
    Then it succeeds
    And source ".mysh/bin/nu" exists
    And the shim ".mysh/bin/nu" invokes mise with specifier "nushell"
    And the shim ".mysh/bin/nu" is marked eager

  Scenario: package add on a duplicate specifier errors without modifying source
    Given I ran "add ripgrep"
    And I record the state of the source tree
    When I run "add ripgrep"
    Then it fails mentioning "already"
    And no file under the source changed
