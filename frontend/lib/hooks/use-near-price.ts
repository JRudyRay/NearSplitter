import { useEffect, useState } from 'react';

interface NearPrice {
  usd: number;
  lastUpdated: number;
}

export function useNearPrice(refreshInterval = 60000): NearPrice | null {
  const [price, setPrice] = useState<NearPrice | null>(null);

  useEffect(() => {
    const fetchPrice = async () => {
      try {
        const response = await fetch(
          'https://api.coingecko.com/api/v3/simple/price?ids=near&vs_currencies=usd'
        );
        const data = await response.json();
        if (data?.near?.usd) {
          setPrice({
            usd: data.near.usd,
            lastUpdated: Date.now(),
          });
        }
      } catch (error) {
        console.error('Failed to fetch NEAR price:', error);
      }
    };

    fetchPrice();
    const interval = setInterval(fetchPrice, refreshInterval);

    return () => clearInterval(interval);
  }, [refreshInterval]);

  return price;
}
