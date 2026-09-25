# Security policy

Sweepr deletes files. For this project, a security problem is anything that could make it delete, move or expose something it should not, for example:

- a path outside the folders a rule allows, or a protected folder such as Documents, reaching deletion;
- a symbolic link, a crafted folder name or an unusual path leading a deletion elsewhere;
- the git guards missing tracked files, a `.env` file or a nested repository;
- a way to run a command that is not in the catalog, or to change its arguments;
- an item deleted without the confirmation its risk level requires.

## Reporting

Use GitHub's private vulnerability reporting: open the **Security** tab of this repository and choose **Report a vulnerability**. It creates a private advisory that only the maintainer can see. Do not open a public issue or pull request.

Include what you found, how to reproduce it (a script that builds the folders in a temporary directory is ideal), and which version or commit you tested. You will get an acknowledgement within a week.

## Supported versions

Only the latest release receives fixes. They ship as a new release, and the advisory is published once the fix is out.
