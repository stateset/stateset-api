## StablePay Crypto - USDC & USDT Payments

### Overview

StablePay Crypto adds instant stablecoin payment support to StablePay, enabling merchants to accept **USDC** and **USDT** payments across multiple blockchains with ultra-low fees and instant settlement.

### Why Stablecoins?

**Traditional payments are slow and expensive. Stablecoins are fast and cheap.**

| Feature | Credit Cards | Wire Transfer | **StablePay Crypto** |
|---------|--------------|---------------|----------------------|
| **Fees** | 2.9% - 3.49% | $25 - $50 | **0.5%** |
| **Settlement** | 3-7 days | 1-3 days | **Minutes** |
| **Chargebacks** | Yes (costly) | No | **No** |
| **Global** | Limited | Limited | **Unlimited** |
| **Availability** | Business hours | Business hours | **24/7** |
| **Minimum** | $0.50 | $1,000+ | **$0.01** |

### Supported Stablecoins

✅ **USDC** (USD Coin) - Circle  
✅ **USDT** (Tether) - Tether Limited

Both are 1:1 backed by US dollars and maintain ~$1.00 value.

### Supported Blockchains

StablePay Crypto supports 5 major blockchains:

#### 1. **Ethereum Mainnet**
- **Use Case**: Large transactions, maximum security
- **Confirmation Time**: ~3 minutes
- **Gas Fees**: $5 - $20
- **Best For**: Transactions > $10,000

#### 2. **Polygon** (Recommended)
- **Use Case**: Retail payments, high volume
- **Confirmation Time**: ~5 seconds
- **Gas Fees**: ~$0.01
- **Best For**: Transactions $10 - $10,000
- **Why**: Ultra-fast, ultra-cheap, perfect for retail

#### 3. **Arbitrum One**
- **Use Case**: Fast, cost-effective payments
- **Confirmation Time**: Instant (1 confirmation)
- **Gas Fees**: ~$0.10
- **Best For**: All transaction sizes
- **Why**: Instant finality, Ethereum security

#### 4. **Optimism**
- **Use Case**: DeFi integration, fast payments
- **Confirmation Time**: ~2 seconds
- **Gas Fees**: ~$0.15
- **Best For**: Transactions $100+

#### 5. **Base** (Coinbase L2)
- **Use Case**: Easy fiat on/off ramps
- **Confirmation Time**: ~2 seconds
- **Gas Fees**: ~$0.10
- **Best For**: Coinbase users, beginners
- **Why**: Native Coinbase integration, simple UX

### API Endpoints

All crypto endpoints are at `/api/v1/stablepay/crypto`:

```
POST   /crypto/payments               Create crypto payment
POST   /crypto/wallets                Add crypto wallet
GET    /crypto/customers/:id/wallets  List customer wallets
GET    /crypto/blockchains            Get supported blockchains
GET    /crypto/health                 Health check
```

### Quick Start

#### 1. Create a Crypto Payment

```bash
curl -X POST http://localhost:8000/api/v1/stablepay/crypto/payments \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "amount": "299.99",
    "token_symbol": "USDC",
    "blockchain": "polygon",
    "from_address": "0x1234567890123456789012345678901234567890",
    "description": "Product Purchase"
  }'
```

**Response:**
```json
{
  "success": true,
  "data": {
    "payment_id": "770e8400-e29b-41d4-a716-446655440000",
    "transaction_number": "CRYPTO-20251013-A1B2C3D4",
    "crypto_transaction_id": "880e8400-e29b-41d4-a716-446655440000",
    "amount": "299.99",
    "token_symbol": "USDC",
    "blockchain": "polygon",
    "network": "mainnet",
    "to_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    "status": "pending",
    "confirmations": 0,
    "required_confirmations": 128,
    "confirmation_progress": "0",
    "estimated_confirmation_time_minutes": 3,
    "gas_estimate_usd": "0.01",
    "total_cost_usd": "300.00",
    "explorer_url": "https://polygonscan.com/address/0x742d35...",
    "created_at": "2025-10-13T12:00:00Z"
  }
}
```

#### 2. Add a Crypto Wallet

```bash
curl -X POST http://localhost:8000/api/v1/stablepay/crypto/wallets \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "wallet_address": "0x1234567890123456789012345678901234567890",
    "blockchain": "polygon",
    "label": "My MetaMask Wallet",
    "set_as_default": true
  }'
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "990e8400-e29b-41d4-a716-446655440000",
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "wallet_address": "0x1234567890123456789012345678901234567890",
    "short_address": "0x1234...7890",
    "blockchain": "polygon",
    "wallet_type": "non_custodial",
    "label": "My MetaMask Wallet",
    "is_verified": false,
    "is_default": true,
    "created_at": "2025-10-13T12:00:00Z"
  }
}
```

#### 3. Get Supported Blockchains

```bash
curl http://localhost:8000/api/v1/stablepay/crypto/blockchains
```

### Integration Examples

#### Node.js/TypeScript

```typescript
import axios from 'axios';

const stablepay = axios.create({
  baseURL: 'http://localhost:8000/api/v1/stablepay/crypto'
});

// Create crypto payment
async function createCryptoPayment() {
  const response = await stablepay.post('/payments', {
    customer_id: '550e8400-e29b-41d4-a716-446655440000',
    amount: '299.99',
    token_symbol: 'USDC',
    blockchain: 'polygon',
    from_address: '0x1234567890123456789012345678901234567890',
    description: 'Product Purchase'
  });
  
  return response.data.data;
}

// Add wallet
async function addWallet(customerId: string, walletAddress: string) {
  const response = await stablepay.post('/wallets', {
    customer_id: customerId,
    wallet_address: walletAddress,
    blockchain: 'polygon',
    label: 'MetaMask',
    set_as_default: true
  });
  
  return response.data.data;
}
```

#### Python

```python
import requests

class StablePayCrypto:
    def __init__(self, base_url="http://localhost:8000/api/v1/stablepay/crypto"):
        self.base_url = base_url
    
    def create_payment(self, customer_id, amount, token, blockchain, from_address):
        response = requests.post(
            f"{self.base_url}/payments",
            json={
                "customer_id": customer_id,
                "amount": str(amount),
                "token_symbol": token,
                "blockchain": blockchain,
                "from_address": from_address,
                "description": "Payment"
            }
        )
        return response.json()["data"]
    
    def add_wallet(self, customer_id, wallet_address, blockchain="polygon"):
        response = requests.post(
            f"{self.base_url}/wallets",
            json={
                "customer_id": customer_id,
                "wallet_address": wallet_address,
                "blockchain": blockchain,
                "set_as_default": True
            }
        )
        return response.json()["data"]

# Usage
client = StablePayCrypto()

# Create payment
payment = client.create_payment(
    customer_id="550e8400-e29b-41d4-a716-446655440000",
    amount=299.99,
    token="USDC",
    blockchain="polygon",
    from_address="0x1234567890123456789012345678901234567890"
)

print(f"Payment created: {payment['payment_id']}")
```

### Frontend Integration (React + Web3)

```typescript
import { ethers } from 'ethers';
import { useState } from 'react';

// USDC contract ABI (ERC20 transfer)
const ERC20_ABI = [
  "function transfer(address to, uint256 amount) returns (bool)"
];

function CryptoCheckout() {
  const [loading, setLoading] = useState(false);
  
  async function payWithUSDC(amount: string, merchantAddress: string) {
    setLoading(true);
    
    try {
      // Connect wallet
      const provider = new ethers.providers.Web3Provider(window.ethereum);
      await provider.send("eth_requestAccounts", []);
      const signer = provider.getSigner();
      
      // USDC contract on Polygon
      const usdcAddress = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
      const usdc = new ethers.Contract(usdcAddress, ERC20_ABI, signer);
      
      // Convert amount to smallest unit (6 decimals for USDC)
      const amountWei = ethers.utils.parseUnits(amount, 6);
      
      // Send transaction
      const tx = await usdc.transfer(merchantAddress, amountWei);
      console.log("Transaction sent:", tx.hash);
      
      // Wait for confirmation
      await tx.wait();
      console.log("Transaction confirmed!");
      
      // Notify backend
      await notifyBackend(tx.hash);
      
      return tx.hash;
    } catch (error) {
      console.error("Payment failed:", error);
      throw error;
    } finally {
      setLoading(false);
    }
  }
  
  async function notifyBackend(txHash: string) {
    // Update payment status in backend
    await fetch('/api/payment-completed', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ transaction_hash: txHash })
    });
  }
  
  return (
    <button 
      onClick={() => payWithUSDC("299.99", "0x742d35...")}
      disabled={loading}
    >
      {loading ? 'Processing...' : 'Pay with USDC'}
    </button>
  );
}
```

### Cost Comparison

#### Example: $1,000 Transaction

| Method | Fee | You Receive | Savings vs Card |
|--------|-----|-------------|-----------------|
| **StablePay Crypto (Polygon)** | **$5.01** | **$994.99** | **$23.99 (83%)** |
| Credit Card (2.9%) | $29.30 | $970.70 | - |
| PayPal (3.49%) | $35.39 | $964.61 | - |

#### Annual Savings Calculator

| Monthly Transactions | Avg Amount | Traditional Cost | Crypto Cost | **Annual Savings** |
|---------------------|------------|------------------|-------------|-------------------|
| 100 | $500 | $17,400 | $3,000 | **$14,400** |
| 1,000 | $500 | $174,000 | $30,000 | **$144,000** |
| 10,000 | $500 | $1,740,000 | $300,000 | **$1,440,000** |

### Benefits

#### For Merchants

✅ **83% Lower Fees** - 0.5% vs 2.9%  
✅ **Instant Settlement** - Minutes vs days  
✅ **No Chargebacks** - Irreversible transactions  
✅ **Global Access** - Accept payments from anywhere  
✅ **24/7 Availability** - No downtime  
✅ **No Currency Conversion** - USD stablecoins worldwide  

#### For Customers

✅ **Fast Checkout** - One-click with Web3 wallet  
✅ **Privacy** - No credit card info needed  
✅ **Security** - Self-custody of funds  
✅ **Global** - Pay from anywhere  
✅ **Rewards** - Potential crypto rewards  

### Smart Contract Addresses

#### USDC

| Blockchain | Contract Address |
|------------|------------------|
| Ethereum | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` |
| Polygon | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` |
| Arbitrum | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` |
| Optimism | `0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85` |
| Base | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |

#### USDT

| Blockchain | Contract Address |
|------------|------------------|
| Ethereum | `0xdAC17F958D2ee523a2206206994597C13D831ec7` |
| Polygon | `0xc2132D05D31c914a87C6611C10748AEb04B58e8F` |
| Arbitrum | `0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9` |
| Optimism | `0x94b008aA00579c1307B0EF2c499aD98a8ce58e58` |

### Security

#### Best Practices

1. **Verify Addresses** - Always double-check wallet addresses
2. **Use Hardware Wallets** - For large amounts
3. **Test with Small Amounts** - First transaction should be small
4. **Monitor Confirmations** - Wait for required confirmations
5. **Keep Private Keys Safe** - Never share with anyone

#### Smart Contract Audits

All token contracts used by StablePay Crypto are:
- ✅ Audited by major security firms
- ✅ Battle-tested with billions in volume
- ✅ Maintained by reputable organizations
- ✅ Open-source and verifiable

### FAQ

**Q: Are stablecoin payments reversible?**  
A: No, blockchain transactions are irreversible. This eliminates chargeback fraud.

**Q: How long do confirmations take?**  
A: Depends on blockchain:
- Polygon: ~5 seconds
- Arbitrum: Instant (1 confirmation)
- Ethereum: ~3 minutes

**Q: What if the transaction fails?**  
A: Failed transactions are refunded automatically. Only gas fees are lost (minimal on L2s).

**Q: Do customers need crypto?**  
A: Yes, customers need USDC or USDT in their wallet. They can buy it on Coinbase, Kraken, etc.

**Q: Can I accept both crypto and cards?**  
A: Yes! StablePay supports both traditional payments and crypto.

**Q: What about taxes?**  
A: Stablecoin payments are taxed the same as cash. Consult your accountant for specifics.

### Demo

Run the comprehensive crypto demo:

```bash
./demos/stablepay_crypto_demo.sh
```

This demonstrates:
- Creating USDC/USDT payments
- Multi-blockchain support
- Wallet management
- Cost comparisons

### Roadmap

#### Q4 2025
- [ ] More stablecoins (DAI, FRAX, BUSD)
- [ ] Solana support
- [ ] Automatic USDC to fiat conversion
- [ ] Recurring crypto payments

#### Q1 2026
- [ ] Bitcoin Lightning Network
- [ ] Ethereum staking integration
- [ ] DeFi yield on idle balances
- [ ] Crypto rewards program

### Support

Need help with crypto payments?

- **Documentation**: This file + [STABLEPAY.md](./STABLEPAY.md)
- **Demo**: `./demos/stablepay_crypto_demo.sh`
- **Email**: crypto@stateset.io

### Additional Resources

- [Polygon Documentation](https://docs.polygon.technology/)
- [Arbitrum Documentation](https://docs.arbitrum.io/)
- [Base Documentation](https://docs.base.org/)
- [USDC Documentation](https://www.circle.com/en/usdc)
- [MetaMask Guide](https://metamask.io/faqs/)

---

**Ready to accept stablecoin payments?**

Save 83% on fees and get instant settlements with StablePay Crypto! 🚀

Start with: `./demos/stablepay_crypto_demo.sh`

