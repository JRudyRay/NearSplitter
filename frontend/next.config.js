/** @type {import('next').NextConfig} */
const nextConfig = {
  // Enable static export for GitHub Pages
  output: 'export',
  
  // Set base path for GitHub Pages (repository name)
  basePath: process.env.NODE_ENV === 'production' ? '/NearSplitter-git' : '',
  
  // Disable image optimization for static export
  images: {
    unoptimized: true,
  },
  
  // Typed routes (moved from experimental)
  typedRoutes: true,
  
  // Trailing slash for better static hosting compatibility
  trailingSlash: true,
};

module.exports = nextConfig;
