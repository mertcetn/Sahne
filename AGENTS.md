# Project Working Rules

This file must contain only rules for working on this project. Under no circumstances may it contain architectural designs, code snippets, or similar implementation material.

## Task Scope

- Keep changes within the requested task scope and avoid unrelated modifications.
- Preserve the user's existing work and uncommitted changes.

## Documentation

- Document every user-facing part of the project in clear, understandable language.
- Write all project documentation in English.
- Store all project documentation in `docs/` or its subdirectories.
- Whenever the architecture changes, including additions or removals, update `docs/architecture.md` to reflect the change.
- Do not describe untested platforms or features as supported.

## Version Control and Releases

- Use Git and GitHub for version control.
- Maintain exactly two branches: `main` and `release`.
- Every commit pushed to `release` must automatically trigger a GitHub Actions workflow that builds the project and publishes the resulting artifacts through GitHub Releases.
- After every task, ask the user to test the changes. Create a commit only after the user approves the changes following testing.
- Provide concrete testing or review steps. Never treat the user's silence as approval.
- Without an explicit user instruction, do not discard user changes, rewrite Git history, force-push, or push to `release`.

## Validation and Task Delivery

- Run builds, tests, and checks appropriate to the changes. Clearly report any checks that could not be run and explain why.
- At task completion, briefly describe the changes, validation results, and how the user can test or review them.

## Security and Dependencies

- Do not weaken sandbox isolation, resource limits, or input validation to work around a problem.
- Before adding or updating a dependency, assess its necessity, maintenance status, license, target-platform compatibility, and security.
- Evaluate the proposed dependency version and its transitive dependencies for known vulnerabilities, relevant security advisories, source and publisher trustworthiness, and security maintenance practices using current information.
- Document material security risks and unresolved uncertainty. Do not assume a dependency is safe merely because no known vulnerabilities were found.

## Technology Documentation

- Research the current documentation for the technologies used while working on the project, and base the work on that documentation.
- Documentation that will be consulted more than once may be saved under `.agents/`. Record the date on which it was retrieved.
- The `.agents/` allowance applies to reference documentation; project documentation must remain under `docs/`.
