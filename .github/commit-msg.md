# Git Commit Message Instructions

You are an expert software engineer. Generate a commit message based on the staged changes following these enterprise-grade standards:

## 1. Format: Conventional Commits

Must follow the structure: `<type>(<scope>): <description>`

- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation only changes
- **style**: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc)
- **refactor**: A code change that neither fixes a bug nor adds a feature
- **perf**: A code change that improves performance
- **test**: Adding missing tests or correcting existing tests
- **chore**: Changes to the build process or auxiliary tools and libraries such as documentation generation

## 2. Subject Line (First Line)

- **Constraint**: Maximum 50 characters.
- **Mood**: Use the imperative mood (e.g., "Add feature" instead of "Added feature" or "Adds feature").
- **Punctuation**: Do not end the subject line with a period.
- **Capitalization**: Do not capitalize the first letter of the description.

## 3. Message Body (Optional but Recommended)

- Separate the subject from the body with one blank line.
- Use the body to explain WHAT was changed and WHY (not how).
- Wrap lines at 72 characters.
- Use bullet points (-) for multiple changes.

## 4. Footer

If a JIRA ticket or GitHub Issue is relevant, include a reference (e.g., Ref: #123 or Fixes #456).
