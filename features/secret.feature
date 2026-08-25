Feature: Secrets are encrypted in Source, plaintext only in Target
  A Secret is a Source file ending in .age, encrypted with a key derived from
  the single shared Passphrase (ADR-0003). It is decrypted only during
  Apply/Diff, written with restrictive permissions, and diffed
  plaintext-to-plaintext — never ciphertext-to-plaintext.

  Background:
    Given a bare remote

  Scenario: apply decrypts a secret and writes restrictive permissions
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    When I run "apply"
    Then it succeeds
    And target ".netrc" contains exactly "machine x login y"
    And target ".netrc" has permissions "600"

  Scenario: diff compares decrypted source against target plaintext
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    And target file ".netrc" is hand-edited to "machine x login z"
    When I run "diff"
    Then the output reports target drift for ".netrc"
    And the output does not contain ciphertext

  Scenario: save on an edited secret re-encrypts into source
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    And target file ".netrc" is hand-edited to "machine x login z"
    When I run "save" answering "y"
    Then it succeeds
    And source ".netrc.age" does not contain the plaintext "machine x login z"
    And re-rendering ".netrc" from source yields exactly "machine x login z"

  Scenario: reset on a secret re-decrypts source into target
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    And target file ".netrc" is hand-edited to "machine x login z"
    When I run "reset" answering "y"
    Then it succeeds
    And target ".netrc" contains exactly "machine x login y"

  Scenario: diff --quick reports a secret clean right after apply
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    When I run "diff --quick"
    Then the output reports no drift

  Scenario: diff --quick detects a hand-edited secret via its cached fingerprint
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    And target file ".netrc" is hand-edited to "machine x login z"
    When I run "diff --quick"
    Then the output reports target drift for ".netrc"

  Scenario: diff --quick reports clean again once save re-establishes the fingerprint
    Given source secret ".netrc.age" encrypting "machine x login y" committed and pushed
    And I ran "apply"
    And target file ".netrc" is hand-edited to "machine x login z"
    And I ran "save" answering "y"
    When I run "diff --quick"
    Then the output reports no drift
