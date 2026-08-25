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

  Scenario: confirmed save also captures a directory-mode new file
    Given source directory ".config/app" tracked with an empty ".track" marker
    And source file ".config/app/base.toml" with content "a=1" committed and pushed
    And I ran "apply"
    And target file ".config/app/extra.toml" already exists with content "b=2"
    When I run "save" answering "y"
    Then it succeeds
    And source ".config/app/extra.toml" contains exactly "b=2"
    And the remote's latest commit contains ".config/app/extra.toml" with "b=2"

  Scenario: confirmed save also pushes a secret added with no target drift
    Given target file ".netrc" already exists with content "machine x login y"
    And I ran "add --secret .netrc"
    When I run "save" answering "y"
    Then it succeeds
    And the remote's latest commit contains ".netrc.age"
