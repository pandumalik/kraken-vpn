const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

function runCommand(command) {
  try {
    return execSync(command, { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }).trim();
  } catch (error) {
    return (error.stdout || '').trim();
  }
}

function parseSemver(versionStr) {
  const parts = versionStr.split('.').map(p => parseInt(p, 10));
  if (parts.length !== 3 || parts.some(isNaN)) {
    // Fallback if version doesn't follow standard X.Y.Z
    return { major: 1, minor: 0, patch: 0 };
  }
  return { major: parts[0], minor: parts[1], patch: parts[2] };
}

function formatSemver({ major, minor, patch }) {
  return `${major}.${minor}.${patch}`;
}

function main() {
  const rootDir = path.resolve(__dirname, '../..');
  const pkgPath = path.join(rootDir, 'package.json');
  const pkgLockPath = path.join(rootDir, 'package-lock.json');

  if (!fs.existsSync(pkgPath)) {
    console.error('package.json not found!');
    process.exit(1);
  }

  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
  let currentVersion = pkg.version || '1.0.0';
  const initialVersion = currentVersion;

  // Fetch tags from remote to ensure full tag history is present
  runCommand('git fetch --tags');

  // Get list of existing git tags
  const existingTagsOutput = runCommand('git tag -l');
  const existingTags = new Set(
    existingTagsOutput
      .split('\n')
      .map(t => t.trim())
      .filter(Boolean)
  );

  console.log(`Initial version in package.json: ${currentVersion}`);
  console.log(`Found ${existingTags.size} existing git tags.`);

  let tagExists = existingTags.has(`v${currentVersion}`) || existingTags.has(currentVersion);

  if (tagExists) {
    console.log(`Tag v${currentVersion} already exists! Auto-incrementing version...`);
    let semverObj = parseSemver(currentVersion);

    while (tagExists) {
      semverObj.patch += 1;
      currentVersion = formatSemver(semverObj);
      tagExists = existingTags.has(`v${currentVersion}`) || existingTags.has(currentVersion);
    }
    console.log(`New version determined: ${currentVersion}`);
  } else {
    console.log(`Version ${currentVersion} is new. No bump required.`);
  }

  const targetTag = `v${currentVersion}`;
  const versionBumped = currentVersion !== initialVersion;

  if (versionBumped) {
    // Update package.json
    pkg.version = currentVersion;
    fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');
    console.log(`Updated package.json version to ${currentVersion}`);

    // Update package-lock.json if it exists
    if (fs.existsSync(pkgLockPath)) {
      try {
        const pkgLock = JSON.parse(fs.readFileSync(pkgLockPath, 'utf8'));
        pkgLock.version = currentVersion;
        if (pkgLock.packages && pkgLock.packages['']) {
          pkgLock.packages[''].version = currentVersion;
        }
        fs.writeFileSync(pkgLockPath, JSON.stringify(pkgLock, null, 2) + '\n', 'utf8');
        console.log(`Updated package-lock.json version to ${currentVersion}`);
      } catch (err) {
        console.warn('Could not update package-lock.json:', err.message);
      }
    }

    // Configure git user
    runCommand('git config user.name "github-actions[bot]"');
    runCommand('git config user.email "github-actions[bot]@users.noreply.github.com"');

    // Stage and commit package files
    runCommand('git add package.json package-lock.json');
    runCommand(`git commit -m "chore(release): bump version to ${currentVersion} [skip ci]"`);

    // Push commit to current branch
    const currentBranch = process.env.GITHUB_REF_NAME || runCommand('git rev-parse --abbrev-ref HEAD') || 'main';
    console.log(`Pushing updated package.json to branch ${currentBranch}...`);
    try {
      execSync(`git push origin HEAD:${currentBranch}`, { stdio: 'inherit' });
    } catch (pushErr) {
      console.warn('Could not push commit to remote (may be local test or permissions):', pushErr.message);
    }
  }

  // Tag commit and push tag
  console.log(`Creating git tag ${targetTag}...`);
  runCommand(`git tag ${targetTag}`);
  console.log(`Pushing git tag ${targetTag} to origin...`);
  try {
    execSync(`git push origin ${targetTag}`, { stdio: 'inherit' });
  } catch (tagErr) {
    console.warn('Could not push tag to remote (may be local test or permissions):', tagErr.message);
  }

  // Write outputs for GitHub Actions step
  const githubOutput = process.env.GITHUB_OUTPUT;
  if (githubOutput) {
    fs.appendFileSync(githubOutput, `version=${currentVersion}\n`, 'utf8');
    fs.appendFileSync(githubOutput, `tag=${targetTag}\n`, 'utf8');
    fs.appendFileSync(githubOutput, `version_bumped=${versionBumped}\n`, 'utf8');
  }

  console.log(`GITHUB_OUTPUT set: version=${currentVersion}, tag=${targetTag}`);
}

main();
