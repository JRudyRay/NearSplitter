/** @type {import('next').NextConfig} */
const isProd = process.env.NODE_ENV === 'production';

// GitHub Actions exposes owner/repo in GITHUB_REPOSITORY.
// For repo pages, GitHub Pages serves the site under /<repo>.
const githubRepoName = process.env.GITHUB_REPOSITORY?.split('/')?.[1];

// Allow overriding (useful for custom domains or local prod builds).
const configuredBasePath = process.env.NEXT_PUBLIC_BASE_PATH;

const basePath =
  configuredBasePath ?? (isProd && githubRepoName ? `/${githubRepoName}` : '');

const nextConfig = {
  // Enable static export for GitHub Pages
  output: 'export',

  // GitHub Pages repo base path support
  basePath,
  assetPrefix: basePath || undefined,

  // Disable image optimization for static export
  images: {
    unoptimized: true,
  },

  // Typed routes disabled – basePath prefixing returns dynamic strings
  // typedRoutes: true,

  // Trailing slash for better static hosting compatibility
  trailingSlash: true,
};

module.exports = nextConfig;
