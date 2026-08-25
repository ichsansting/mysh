Feature: Diff reports drift across Target, Source, and Remote
  The three-state model: Target (live disk), Source (local working tree),
  Remote (canonical shared history). Diff reports both axes at once and,
  for .track-marked directories, scans for new and missing files.

  Background:
    Given a bare remote

  Scenario: no drift anywhere produces clean output
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    When I run "diff"
    Then it succeeds
    And the output reports no drift

  Scenario: reports target vs source drift only
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    And target file ".bashrc" is hand-edited to "alias l=ls -la"
    When I run "diff"
    Then the output reports target drift for ".bashrc"
    And the output reports no remote drift

  Scenario: reports source ahead of remote only
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    And source file ".bashrc" is edited to "alias l=ls -la" and committed but not pushed
    And I ran "apply"
    When I run "diff"
    Then the output reports drift ahead of remote for ".bashrc"
    And the output reports no target drift

  Scenario: reports ahead drift for a file not yet pushed
    Given source file ".bashrc" with content "alias l=ls" committed but not pushed
    And I ran "apply"
    When I run "diff"
    Then the output reports drift ahead of remote for ".bashrc"

  Scenario: reports behind drift for a file only on remote
    Given another device pushed file ".vimrc" with content "set number"
    When I run "diff"
    Then the output reports drift behind remote for ".vimrc"

  Scenario: reports diverged drift when both sides changed the same path
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    And source file ".bashrc" is edited to "alias l=ls -la" and committed but not pushed
    And another device pushed file ".bashrc" with content "alias l=ls --color"
    When I run "diff"
    Then the output reports diverged drift for ".bashrc"

  Scenario: reports both drifts together distinguishing sides
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    And target file ".bashrc" is hand-edited to "alias l=ls -la"
    And another device pushed file ".vimrc" with content "set number"
    When I run "diff"
    Then the output reports target drift for ".bashrc"
    And the output reports drift behind remote for ".vimrc"

  Scenario: diff --quick never contacts the network
    Given source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"
    And another device pushed file ".vimrc" with content "set number"
    When I run "diff --quick"
    Then the output reports no drift
    When I run "diff"
    Then the output reports drift behind remote for ".vimrc"

  @dirmode
  Scenario: a .track-marked directory flags a new file present only in target
    Given source directory ".config/app" tracked with an empty ".track" marker
    And source file ".config/app/base.toml" with content "a=1" committed and pushed
    And I ran "apply"
    And target file ".config/app/extra.toml" already exists with content "b=2"
    When I run "diff"
    Then the output flags ".config/app/extra.toml" as new

  @dirmode
  Scenario: a .track-marked directory flags a file missing from target
    Given source directory ".config/app" tracked with an empty ".track" marker
    And source file ".config/app/base.toml" with content "a=1" committed and pushed
    And I ran "apply"
    And target file ".config/app/base.toml" is deleted
    When I run "diff"
    Then the output flags ".config/app/base.toml" as missing

  @dirmode
  Scenario: a .track ignore pattern excludes matching files from the new-file scan
    Given source directory ".config/app" tracked with a ".track" marker containing "*.log"
    And source file ".config/app/base.toml" with content "a=1" committed and pushed
    And I ran "apply"
    And target file ".config/app/debug.log" already exists with content "noise"
    And target file ".config/app/extra.toml" already exists with content "b=2"
    When I run "diff"
    Then the output flags ".config/app/extra.toml" as new
    And the output does not mention ".config/app/debug.log"

  @dirmode
  Scenario: a directory without .track never scans for new sibling files
    Given source file ".config/app/base.toml" with content "a=1" committed and pushed
    And I ran "apply"
    And target file ".config/app/sibling.toml" already exists with content "b=2"
    When I run "diff"
    Then the output does not mention ".config/app/sibling.toml"
