# Security policy

## Supported versions

Security fixes are evaluated against the current default branch and, when a
release exists, the latest release. Older commits or releases are not promised
backports or a fixed support window. Include the affected commit or version in
the report so maintainers can assess applicability.

## Report a vulnerability privately

Use GitHub's private advisory form:

<https://github.com/ianmoran11/sembla/security/advisories/new>

Include:

- the affected commit, version, component, and environment;
- the security impact and realistic threat scenario;
- minimal reproduction steps or a proof of concept;
- any known preconditions, mitigations, or suggested fix; and
- whether the issue or proof has been disclosed elsewhere.

Keep sensitive details in the private advisory. Never commit cloud or API
credentials, Terraform state or plan files, console passwords, or raw agent
transcripts. Redact secrets from logs and reproduction artifacts before
attaching them to the advisory.
