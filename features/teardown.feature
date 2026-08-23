Feature: Teardown returns the device to its pre-mysh state
  Teardown replays the Application Log: deletes files mysh created, restores
  backed-up originals, removes the isolated mise prefix (all packages with
  it), strips rc-file lines and the bootstrap footprint. One deliberate
  exception (ADR-0008): a partially-owned Overlay target is left in place.

  Scenario: teardown deletes created files and restores overwritten originals
    Given target file ".gitconfig" already exists with content "pre-mysh"
    And source file ".gitconfig" with content "managed"
    And source file ".bashrc" with content "alias l=ls"
    And I ran "apply"
    When I run "teardown" answering "y"
    Then it succeeds
    And target ".bashrc" does not exist
    And target ".gitconfig" contains exactly "pre-mysh"
    And no mysh residue remains under the target

  @overlay
  Scenario: teardown leaves overlay targets in place with accumulated keys intact
    Given target file ".claude.json" already exists with content "{\"hasCompletedOnboarding\": false}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    And I ran "apply"
    And target file ".claude.json" accumulates key "numStartups" with value 42
    When I run "teardown" answering "y"
    Then it succeeds
    And the summary says ".claude.json" is left in place
    And target ".claude.json" as JSON has "hasCompletedOnboarding" equal to true
    And target ".claude.json" as JSON has "numStartups" equal to 42

  @overlay
  Scenario: teardown leaves an overlay-created file in place too
    Given source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    And I ran "apply"
    When I run "teardown" answering "y"
    Then target ".claude.json" as JSON has "hasCompletedOnboarding" equal to true

  Scenario: declined teardown leaves target and log unchanged
    Given source file ".bashrc" with content "alias l=ls"
    And I ran "apply"
    And I record the state of the target tree
    When I run "teardown" answering "n"
    Then it succeeds
    And no file under the target changed

  Scenario: teardown on a device mysh never touched is a noop
    When I run "teardown" answering "y"
    Then it succeeds
    And no mysh residue remains under the target

  @package
  Scenario: teardown removes packages, bootstrapped mise, and shims
    Given no mise resolvable on PATH
    And a stubbed curl that installs a recording mise
    And source eager shim ".mysh/bin/fish" for specifier "fish"
    And source lazy shim ".mysh/bin/rg" for specifier "ripgrep"
    And I ran "apply"
    When I run "teardown" answering "y"
    Then it succeeds
    And the mysh-owned mise binary does not exist
    And the mise data directory does not exist
    And target ".mysh/bin/rg" does not exist

  @package
  Scenario: a full bootstrap-to-teardown cycle leaves no residue
    Given a bare remote whose profile contains file ".bashrc" with content "alias l=ls"
    And a stubbed curl that downloads the real mysh binary
    And an rc file with content "# original rc"
    When I bootstrap the device
    And I run "teardown" answering "y"
    Then the rc file contains exactly "# original rc"
    And no mysh residue remains under the target
