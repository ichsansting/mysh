Feature: Save captures live Target edits back into Source
  Local wins. Save shows the pending drift, requires explicit confirmation,
  then copies Target content into Source, commits, and pushes to Remote.

  Background:
    Given a bare remote
    And source file ".bashrc" with content "alias l=ls" committed and pushed
    And I ran "apply"

  Scenario: confirmed save captures target drift, commits, and pushes
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    When I run "save" answering "y"
    Then it succeeds
    And source ".bashrc" contains exactly "alias l=ls -la"
    And the remote's latest commit contains ".bashrc" with "alias l=ls -la"

  Scenario: declined save leaves source, target, and remote unchanged
    Given target file ".bashrc" is hand-edited to "alias l=ls -la"
    And I record the state of the remote
    When I run "save" answering "n"
    Then it succeeds
    And source ".bashrc" contains exactly "alias l=ls"
    And target ".bashrc" contains exactly "alias l=ls -la"
    And the remote is unchanged

  Scenario: save with no target drift is a noop
    Given I record the state of the remote
    When I run "save"
    Then it succeeds
    And the output reports nothing to save
    And the remote is unchanged
