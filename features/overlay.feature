Feature: Overlays enforce declared keys onto a Target mysh does not own
  A <name>.overlay file in Source declares only the keys mysh should enforce
  onto Target's <name> (JSON, shallow merge, key order preserved). Every other
  key in Target is left untouched and never read into Source. Derived-only:
  drift can be re-enforced via apply/reset but never saved back.

  Scenario: apply creates the target file when it does not exist yet
    Given source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "apply"
    Then it succeeds
    And target ".claude.json" as JSON has "hasCompletedOnboarding" equal to true

  Scenario: apply merges declared keys onto an existing target preserving the rest
    Given target file ".claude.json" already exists with content "{\"numStartups\": 42, \"hasCompletedOnboarding\": false}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "apply"
    Then target ".claude.json" as JSON has "hasCompletedOnboarding" equal to true
    And target ".claude.json" as JSON has "numStartups" equal to 42

  Scenario: apply is a noop once declared keys already match
    Given target file ".claude.json" already exists with content "{\"hasCompletedOnboarding\": true}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    And I record the state of the target tree
    When I run "apply"
    Then it succeeds
    And no file under the target changed

  Scenario: diff shows no drift when declared keys match regardless of other keys
    Given target file ".claude.json" already exists with content "{\"numStartups\": 42, \"hasCompletedOnboarding\": true}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "diff"
    Then it succeeds
    And the output does not mention ".claude.json"

  Scenario: diff shows drift when a target value disagrees with the declared one
    Given target file ".claude.json" already exists with content "{\"hasCompletedOnboarding\": false}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "diff"
    Then the output reports target drift for ".claude.json"

  Scenario: diff shows drift when the target file does not exist yet
    Given source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "diff"
    Then the output reports target drift for ".claude.json"

  Scenario: save is rejected for an overlay-enforced target
    Given a bare remote
    And target file ".claude.json" already exists with content "{\"hasCompletedOnboarding\": false}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    And the source is committed and pushed
    When I run "save" answering "y"
    Then it fails mentioning "overlay"

  Scenario: reset discards drift by re-enforcing the declared value
    Given a bare remote
    And target file ".claude.json" already exists with content "{\"numStartups\": 42, \"hasCompletedOnboarding\": false}"
    And source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    And the source is committed and pushed
    When I run "reset" answering "y"
    Then target ".claude.json" as JSON has "hasCompletedOnboarding" equal to true
    And target ".claude.json" as JSON has "numStartups" equal to 42

  Scenario: the overlay file itself is never copied verbatim into target
    Given source overlay ".claude.json.overlay" declaring {"hasCompletedOnboarding": true}
    When I run "apply"
    Then it succeeds
    And target ".claude.json.overlay" does not exist
