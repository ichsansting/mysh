Feature: Packages are mise-managed CLI tools behind real shim files
  Every Package is a portable shim file in Source's .mysh/bin (ADR-0007).
  An eager shim's shebang is "#!/bin/sh": apply collects every eager
  specifier and prewarms them in one batched `mise install`. A lazy shim's
  shebang is "#!/usr/bin/env fish" and installs on first invocation via
  `mise x` (ADR-0015). mise itself is self-bootstrapped into a mysh-owned
  prefix on first need (ADR-0005).

  Scenario: apply bootstraps missing mise and logs it
    Given no mise resolvable on PATH
    And a stubbed curl that installs a recording mise
    And source eager shim ".mysh/bin/fish" for specifier "fish"
    When I run "apply"
    Then it succeeds
    And the mysh-owned mise binary exists
    And the log records the mise bootstrap

  Scenario: a second apply reuses bootstrapped mise without reinvoking the installer
    Given no mise resolvable on PATH
    And a stubbed curl that installs a recording mise
    And source eager shim ".mysh/bin/fish" for specifier "fish"
    And I ran "apply"
    When I run "apply"
    Then it succeeds
    And the curl stub was invoked exactly once

  Scenario: apply does not touch mise when no packages are declared
    Given no mise resolvable on PATH
    And a stubbed curl that installs a recording mise
    And source file ".bashrc" with content "alias l=ls"
    When I run "apply"
    Then it succeeds
    And the curl stub was never invoked
    And the mysh-owned mise binary does not exist

  Scenario: apply bootstraps mise for a lazy-only device so the shim has something to invoke
    Given no mise resolvable on PATH
    And a stubbed curl that installs a recording mise
    And source lazy shim ".mysh/bin/rg" for specifier "ripgrep"
    When I run "apply"
    Then it succeeds
    And the mysh-owned mise binary exists

  Scenario: a lazy shim is identity-copied verbatim and stays executable
    Given a stubbed mise on PATH
    And source lazy shim ".mysh/bin/rg" for specifier "ripgrep"
    When I run "apply"
    Then target ".mysh/bin/rg" is byte-identical to its source shim
    And target ".mysh/bin/rg" is executable

  Scenario: eager packages are batch-installed in one mise install during apply
    Given a stubbed mise on PATH
    And source eager shim ".mysh/bin/fish" for specifier "fish"
    And source eager shim ".mysh/bin/starship" for specifier "starship"
    And source lazy shim ".mysh/bin/rg" for specifier "ripgrep"
    When I run "apply"
    Then it succeeds
    And the mise stub saw exactly one install invocation
    And that install invocation named specifiers "fish" and "starship"
    And that install invocation did not name "ripgrep"

  Scenario: a lazy shim installs then execs on first invocation
    Given a stubbed mise on PATH
    And real fish resolvable on PATH
    And source lazy shim ".mysh/bin/rg" for specifier "ripgrep"
    And I ran "apply"
    When I invoke the rendered shim ".mysh/bin/rg" with argument "hello"
    Then the mise stub saw an x invocation for specifier "ripgrep" running "rg" with "hello"
