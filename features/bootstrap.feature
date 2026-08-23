Feature: One-line bootstrap sets up a brand-new device
  bootstrap.sh detects OS/arch, downloads the prebuilt binary, puts .mysh/bin
  on PATH (logged), exports MISE_DATA_DIR durably, sparse-checks-out only
  profile/ as Source, then hands off to `mysh apply`. Only git and curl are
  assumed present.

  Background:
    Given a bare remote whose profile contains file ".bashrc" with content "alias l=ls"
    And a stubbed curl that downloads the real mysh binary
    And an rc file with content "# original rc"

  Scenario: bootstrap installs the binary, adds PATH, clones source, and hands off
    When I bootstrap the device
    Then it succeeds
    And the mysh binary is installed under the target
    And the rc file adds ".mysh/bin" to PATH
    And the log records the bootstrap install and the PATH addition
    And the source checkout contains only "profile"
    And target ".bashrc" contains exactly "alias l=ls"

  Scenario: bootstrap defaults to mysh's own releases repo with no env override
    Then the bootstrap script's default release download points at "ichsansting/mysh"

  Scenario: rerunning bootstrap does not duplicate the PATH line or log entries
    Given I bootstrapped the device
    When I bootstrap the device
    Then it succeeds
    And the rc file adds ".mysh/bin" to PATH exactly once
    And the log records the bootstrap install exactly once

  Scenario: bootstrap exports MISE_DATA_DIR durably in the rc file
    When I bootstrap the device
    Then it succeeds
    And the rc file exports MISE_DATA_DIR pointing at ".mysh/mise"

  Scenario: every documented command succeeds post-bootstrap with no flags or env
    Given I bootstrapped the device
    When I run every documented command with no flags and HOME set to the target
    Then each of them succeeds
