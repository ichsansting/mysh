Feature: Update discards local drift, re-applies Remote, and refreshes the mysh binary
  Remote wins. Update shows the pending Source-side drift, requires explicit
  confirmation, then forces Source to match Remote and re-renders every
  Target. Refuses outright on a diverged path instead — mysh does no
  three-way merge, so it will not silently pick a side there either. It also
  refreshes the installed mysh binary itself against the current release
  (ADR-0013) — independent of the Source-side outcome above.

  Background:
    Given a bare remote
    And source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"

  Scenario: confirmed update discards local drift and reapplies remote
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    And another device pushed file ".vimrc" with content "set number"
    When I run "update" answering "y"
    Then it succeeds
    And target ".bashrc" contains exactly "alias l=ls"
    And target ".vimrc" contains exactly "set number"

  Scenario: declined update leaves source, target, and remote unchanged
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    When I run "update" answering "n"
    Then it succeeds
    And target ".bashrc" contains exactly "alias l=ls -la"
    And source ".bashrc" contains exactly "alias l=ls"

  Scenario: update with no drift is a noop
    Given I record the state of the target tree
    When I run "update"
    Then it succeeds
    And the output reports nothing to update
    And no file under the target changed

  Scenario: update refuses a diverged path rather than picking a side
    Given source file ".bashrc" is edited to "alias l=ls -la" and committed but not pushed
    And another device pushed file ".bashrc" with content "alias l=ls --color"
    When I run "update" answering "y"
    Then it fails mentioning "diverged"
    And source ".bashrc" contains exactly "alias l=ls -la"

  Scenario: update with no mysh binary installed never touches curl
    When I run "update"
    Then it succeeds
    And the curl stub was never invoked

  Scenario: update replaces an installed binary that differs from the current release
    Given a mysh binary is installed with content "old binary bytes"
    And a stubbed curl that downloads the real mysh binary
    When I run "update"
    Then it succeeds
    And the installed mysh binary matches the real compiled binary

  Scenario: update leaves an installed binary alone when it already matches the release
    Given a mysh binary is installed matching the real compiled binary
    And a stubbed curl that downloads the real mysh binary
    And I record the state of the target tree
    When I run "update"
    Then it succeeds
    And no file under the target changed
