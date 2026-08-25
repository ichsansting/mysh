Feature: Reset discards local drift and re-applies Remote
  Remote wins. Reset shows the pending drift, requires explicit confirmation,
  then forces Source to match Remote and re-renders every Target. Refuses
  outright on a diverged path instead — mysh does no three-way merge, so it
  will not silently pick a side there either.

  Background:
    Given a bare remote
    And source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"

  Scenario: confirmed reset discards local drift and reapplies remote
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    And another device pushed file ".vimrc" with content "set number"
    When I run "reset" answering "y"
    Then it succeeds
    And target ".bashrc" contains exactly "alias l=ls"
    And target ".vimrc" contains exactly "set number"

  Scenario: declined reset leaves source, target, and remote unchanged
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    When I run "reset" answering "n"
    Then it succeeds
    And target ".bashrc" contains exactly "alias l=ls -la"
    And source ".bashrc" contains exactly "alias l=ls"

  Scenario: reset with no drift is a noop
    Given I record the state of the target tree
    When I run "reset"
    Then it succeeds
    And the output reports nothing to reset
    And no file under the target changed

  Scenario: reset refuses a diverged path rather than picking a side
    Given source file ".bashrc" is edited to "alias l=ls -la" and committed but not pushed
    And another device pushed file ".bashrc" with content "alias l=ls --color"
    When I run "reset" answering "y"
    Then it fails mentioning "diverged"
    And source ".bashrc" contains exactly "alias l=ls -la"
