Feature: Apply renders Source into Target
  Apply is the uniform render step (ADR-0002): identity copy for plain files,
  with first-touch bookkeeping in the Application Log so Teardown can reverse it.

  Scenario: apply renders plain files byte for byte
    Given source file ".bashrc" with content "alias l=ls"
    When I run "apply"
    Then it succeeds
    And target ".bashrc" contains exactly "alias l=ls"

  Scenario: apply twice with no changes is a noop
    Given source file ".bashrc" with content "alias l=ls"
    And I ran "apply"
    And I record the state of the target tree
    When I run "apply"
    Then it succeeds
    And no file under the target changed

  Scenario: apply to a fresh path logs full ownership with no backup
    Given source file ".bashrc" with content "alias l=ls"
    When I run "apply"
    Then the log records full ownership of ".bashrc" with no backup

  Scenario: apply to a pre-existing path backs it up and logs the backup
    Given target file ".gitconfig" already exists with content "pre-mysh"
    And source file ".gitconfig" with content "managed"
    When I run "apply"
    Then target ".gitconfig" contains exactly "managed"
    And the log records full ownership of ".gitconfig" with a backup
    And the backup for ".gitconfig" contains exactly "pre-mysh"

  Scenario: reapplying an already managed path does not retrigger a backup
    Given target file ".gitconfig" already exists with content "pre-mysh"
    And source file ".gitconfig" with content "managed"
    And I ran "apply"
    And source file ".gitconfig" with content "managed-v2"
    When I run "apply"
    Then target ".gitconfig" contains exactly "managed-v2"
    And the backup for ".gitconfig" contains exactly "pre-mysh"
    And the log has exactly one entry for ".gitconfig"
