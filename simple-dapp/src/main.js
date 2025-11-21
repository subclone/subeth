import './style.css'

// App state
let currentAccount = null;
let isConnected = false;

// Network configuration
const SUBETH_NETWORK = {
  chainId: '0x2A', // 42
  chainName: 'Subeth Local',
  nativeCurrency: {
    name: 'UNIT',
    symbol: 'UNIT',
    decimals: 18
  },
  rpcUrls: ['http://localhost:8545']
};

// Initialize app
document.querySelector('#app').innerHTML = `
  <div class="container">
    <h1>Subeth Transfer Demo</h1>
    <p class="subtitle">Send tokens using MetaMask on Substrate chains</p>

    <div class="card">
      <div id="connectionStatus" class="status-box disconnected">
        <span class="status-dot"></span>
        <span id="statusText">Not Connected</span>
      </div>

      <button id="connectBtn" class="btn btn-primary">Connect MetaMask</button>

      <div id="accountInfo" class="account-info" style="display: none;">
        <div class="info-row">
          <label>Address:</label>
          <code id="address"></code>
        </div>
        <div class="info-row">
          <label>Balance:</label>
          <span id="balance">0 UNIT</span>
        </div>
        <div class="info-row">
          <label>Chain ID:</label>
          <span id="chainId">-</span>
        </div>
      </div>
    </div>

    <div class="card" id="transferCard" style="display: none;">
      <h2>Send Transfer</h2>

      <div class="form-group">
        <label for="recipient">Recipient Address:</label>
        <input
          type="text"
          id="recipient"
          placeholder="0x..."
          class="input"
        />
      </div>

      <div class="form-group">
        <label for="amount">Amount (in smallest unit):</label>
        <input
          type="text"
          id="amount"
          placeholder="1000000000000"
          class="input"
        />
        <small class="hint">Example: 1000000000000 = 0.000001 UNIT (with 18 decimals)</small>
      </div>

      <button id="sendBtn" class="btn btn-primary">Send Transaction</button>

      <div id="result" class="result" style="display: none;"></div>
    </div>

    <div class="footer">
      <p>Make sure the Subeth adapter is running on <code>http://localhost:8545</code></p>
    </div>
  </div>
`

// Setup event listeners
setupApp();

function setupApp() {
  const connectBtn = document.getElementById('connectBtn');
  const sendBtn = document.getElementById('sendBtn');

  connectBtn.addEventListener('click', connectWallet);
  sendBtn.addEventListener('click', sendTransfer);

  // Check if MetaMask is installed
  if (!window.ethereum) {
    connectBtn.disabled = true;
    connectBtn.textContent = 'MetaMask Not Detected';
    showResult('Please install MetaMask to use this app', 'error');
  }

  // Listen for account changes
  if (window.ethereum) {
    window.ethereum.on('accountsChanged', handleAccountsChanged);
    window.ethereum.on('chainChanged', () => window.location.reload());

    // Check if already connected
    window.ethereum.request({ method: 'eth_accounts' })
      .then(accounts => {
        if (accounts.length > 0) {
          handleAccountsChanged(accounts);
        }
      })
      .catch(console.error);
  }
}

async function connectWallet() {
  const connectBtn = document.getElementById('connectBtn');

  try {
    connectBtn.disabled = true;
    connectBtn.textContent = 'Connecting...';

    // Request accounts
    const accounts = await window.ethereum.request({
      method: 'eth_requestAccounts'
    });

    handleAccountsChanged(accounts);
  } catch (error) {
    console.error('Connection error:', error);
    showResult(`Failed to connect: ${error.message}`, 'error');
    connectBtn.disabled = false;
    connectBtn.textContent = 'Connect MetaMask';
  }
}

async function handleAccountsChanged(accounts) {
  if (accounts.length === 0) {
    // Disconnected
    currentAccount = null;
    isConnected = false;
    updateConnectionStatus(false);
    document.getElementById('accountInfo').style.display = 'none';
    document.getElementById('transferCard').style.display = 'none';
    document.getElementById('connectBtn').disabled = false;
    document.getElementById('connectBtn').textContent = 'Connect MetaMask';
  } else {
    currentAccount = accounts[0];
    isConnected = true;
    updateConnectionStatus(true);
    await updateAccountInfo();
    document.getElementById('transferCard').style.display = 'block';
  }
}

function updateConnectionStatus(connected) {
  const statusBox = document.getElementById('connectionStatus');
  const statusText = document.getElementById('statusText');
  const connectBtn = document.getElementById('connectBtn');

  if (connected) {
    statusBox.className = 'status-box connected';
    statusText.textContent = 'Connected';
    connectBtn.textContent = 'Connected';
    connectBtn.disabled = true;
  } else {
    statusBox.className = 'status-box disconnected';
    statusText.textContent = 'Not Connected';
    connectBtn.textContent = 'Connect MetaMask';
    connectBtn.disabled = false;
  }
}

async function updateAccountInfo() {
  if (!currentAccount) return;

  try {
    // Get balance
    const balance = await window.ethereum.request({
      method: 'eth_getBalance',
      params: [currentAccount, 'latest']
    });

    // Get chain ID
    const chainId = await window.ethereum.request({
      method: 'eth_chainId'
    });

    // Format balance (from wei to readable)
    const balanceInUnits = formatBalance(balance);

    // Update UI
    document.getElementById('address').textContent = currentAccount;
    document.getElementById('balance').textContent = `${balanceInUnits} UNIT`;
    document.getElementById('chainId').textContent = parseInt(chainId, 16);
    document.getElementById('accountInfo').style.display = 'block';
  } catch (error) {
    console.error('Error updating account info:', error);
  }
}

async function sendTransfer() {
  const sendBtn = document.getElementById('sendBtn');
  const recipient = document.getElementById('recipient').value.trim();
  const amount = document.getElementById('amount').value.trim();

  // Validation
  if (!recipient) {
    showResult('Please enter a recipient address', 'error');
    return;
  }

  if (!/^0x[a-fA-F0-9]{40}$/.test(recipient)) {
    showResult('Invalid Ethereum address format', 'error');
    return;
  }

  if (!amount || isNaN(amount) || BigInt(amount) <= 0) {
    showResult('Please enter a valid amount', 'error');
    return;
  }

  try {
    sendBtn.disabled = true;
    sendBtn.textContent = 'Sending...';
    clearResult();

    // Convert amount to hex
    const valueHex = '0x' + BigInt(amount).toString(16);

    // Send transaction
    const txHash = await window.ethereum.request({
      method: 'eth_sendTransaction',
      params: [{
        from: currentAccount,
        to: recipient,
        value: valueHex,
        gas: '0x5208' // 21000
      }]
    });

    showResult(
      `Transaction sent successfully!\n\nTx Hash: ${txHash}\n\nFrom: ${currentAccount}\nTo: ${recipient}\nAmount: ${amount}`,
      'success'
    );

    // Refresh balance after 2 seconds
    setTimeout(() => updateAccountInfo(), 2000);

  } catch (error) {
    console.error('Transfer error:', error);
    showResult(`Transaction failed: ${error.message}`, 'error');
  } finally {
    sendBtn.disabled = false;
    sendBtn.textContent = 'Send Transaction';
  }
}

function formatBalance(hexBalance) {
  const balance = BigInt(hexBalance);
  const divisor = BigInt(10 ** 18);
  const whole = balance / divisor;
  const fraction = balance % divisor;

  const fractionStr = fraction.toString().padStart(18, '0');
  const trimmedFraction = fractionStr.replace(/0+$/, '').slice(0, 6);

  if (trimmedFraction) {
    return `${whole}.${trimmedFraction}`;
  }
  return whole.toString();
}

function showResult(message, type) {
  const resultDiv = document.getElementById('result');
  resultDiv.style.display = 'block';
  resultDiv.className = `result ${type}`;
  resultDiv.innerHTML = `<pre>${message}</pre>`;
}

function clearResult() {
  const resultDiv = document.getElementById('result');
  resultDiv.style.display = 'none';
  resultDiv.innerHTML = '';
}
