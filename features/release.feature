Feature: release.sh publishes the single mutable v1 release
  release.sh provisions its cross-build toolchain (rust, zig, cargo-zigbuild) via
  mise, then cross-builds the static binaries and creates or updates one mutable
  "v1" GitHub release, so bootstrap.sh always has a stable asset URL.

  Scenario: the release script creates then updates the single v1 release idempotently
    Given stubbed build and gh tools that record their invocations
    When I run the release script twice
    Then both runs succeed
    And the first run creates the "v1" release and the second updates it

  Scenario: the release script refuses a dirty working tree
    Given a repo checkout with an uncommitted change
    When I run the release script
    Then it fails mentioning "dirty"

  Scenario: the release script still fails when the cross-build toolchain is broken
    Given a PATH without the cross-build toolchain
    When I run the release script
    Then it fails
