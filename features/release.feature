Feature: release.sh publishes the single mutable v1 release
  release.sh cross-builds the static binaries and creates or updates one
  mutable "v1" GitHub release, so bootstrap.sh always has a stable asset URL.

  Scenario: the release script creates then updates the single v1 release idempotently
    Given stubbed build and gh tools that record their invocations
    When I run the release script twice
    Then both runs succeed
    And the first run creates the "v1" release and the second updates it

  Scenario: the release script refuses a dirty working tree
    Given a repo checkout with an uncommitted change
    When I run the release script
    Then it fails mentioning "dirty"

  Scenario: the release script fails with instructions when a build tool is missing
    Given a PATH without the cross-build toolchain
    When I run the release script
    Then it fails mentioning how to install the missing tool
