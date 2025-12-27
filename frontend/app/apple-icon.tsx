import { ImageResponse } from 'next/og';

export const dynamic = 'force-static';
export const revalidate = false;

export const size = {
  width: 180,
  height: 180,
};

export const contentType = 'image/png';

export default function AppleIcon() {
  // Geometric N with split diagonal
  return new ImageResponse(
    (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: '#111111',
          borderRadius: 36,
        }}
      >
        <svg width="140" height="140" viewBox="0 0 40 40" fill="none">
          {/* Left bar */}
          <rect x="7" y="8" width="5" height="24" rx="1" fill="#e5e7eb" />
          {/* Right bar */}
          <rect x="28" y="8" width="5" height="24" rx="1" fill="#00EC97" />
          {/* Diagonal split */}
          <path d="M12 10 L28 30 L28 26 L12 6 Z" fill="#e5e7eb" opacity="0.7" />
          <path d="M12 14 L28 34 L28 30 L12 10 Z" fill="#00EC97" opacity="0.85" />
        </svg>
      </div>
    ),
    { ...size }
  );
}
