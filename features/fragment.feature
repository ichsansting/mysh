Feature: Fragments compose one Target file from pieces
  A <name>.frag/ directory in Source renders its members, in lexical filename
  order, into a single Target file <name>. Members may themselves be Secrets.
  A composed Target is derived-only: drift shows on diff, reset discards it,
  save refuses it (there is no unambiguous fragment to attribute an edit to).

  Background:
    Given a bare remote

  Scenario: apply concatenates plain and secret fragments in lexical order
    Given source fragment ".gitconfig.frag/10-base" with content "[user]"
    And source fragment secret ".gitconfig.frag/20-token.age" encrypting "token = t"
    And the source is committed and pushed
    When I run "apply"
    Then it succeeds
    And target ".gitconfig" contains exactly "[user]token = t"

  Scenario: a newly added fragment is picked up by the next apply with no registration
    Given source fragment ".gitconfig.frag/10-base" with content "[user]"
    And I ran "apply"
    And source fragment ".gitconfig.frag/20-alias" with content "[alias]"
    When I run "apply"
    Then target ".gitconfig" contains exactly "[user][alias]"

  Scenario: diff shows drift between the live merged file and a fresh render
    Given source fragment ".gitconfig.frag/10-base" with content "[user]"
    And the source is committed and pushed
    And I ran "apply"
    And target file ".gitconfig" is hand-edited to "[user][extra]"
    When I run "diff"
    Then the output reports target drift for ".gitconfig"

  Scenario: save is rejected for a composed target
    Given source fragment ".gitconfig.frag/10-base" with content "[user]"
    And the source is committed and pushed
    And I ran "apply"
    And target file ".gitconfig" is hand-edited to "[user][extra]"
    When I run "save" answering "y"
    Then it fails mentioning "composed"
    And source fragment ".gitconfig.frag/10-base" contains exactly "[user]"

  Scenario: reset discards drift by re-rendering fresh from fragments
    Given source fragment ".gitconfig.frag/10-base" with content "[user]"
    And the source is committed and pushed
    And I ran "apply"
    And target file ".gitconfig" is hand-edited to "[user][extra]"
    When I run "reset" answering "y"
    Then target ".gitconfig" contains exactly "[user]"
