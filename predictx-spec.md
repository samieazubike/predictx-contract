# Football Prediction Market Platform
## Product Specification Document

---

## 📋 Table of Contents
1. [Product Overview](#product-overview)
2. [How It Works](#how-it-works)
3. [User Roles](#user-roles)
4. [Core Features](#core-features)
5. [Technical Architecture](#technical-architecture)
6. [Smart Contract Specifications](#smart-contract-specifications)
7. [Frontend Requirements](#frontend-requirements)
8. [User Flows](#user-flows)
9. [Security & Compliance](#security-compliance)
10. [Development Phases](#development-phases)

---

## 🎯 Product Overview

### What We're Building
A Web3-based prediction market platform where users stake cryptocurrency on specific football match events. This is **NOT a traditional betting platform** - it's a community-driven prediction game with transparent, blockchain-based resolution.

### The Problem We Solve
- Traditional sports betting is opaque and controlled by bookmakers
- Fans want more granular, engaging ways to predict match outcomes
- Existing prediction markets lack social/community elements
- Trust issues with centralized platforms

### Our Solution
A decentralized platform where:
- Users create prediction polls about specific match events
- Community stakes crypto on opposing sides
- Winners split the pool proportionally
- Outcomes verified by community voting + admin oversight
- Full transparency via blockchain

### Key Differentiators
1. **User-Generated Content**: Anyone can create prediction polls
2. **Pool-Based**: Multiple users on each side, proportional rewards
3. **Hybrid Oracle**: Community voting + admin verification for accuracy
4. **Granular Predictions**: Not just "who wins", but specific in-game events
5. **Gaming UI**: Engaging, game-like interface (not boring finance app)

---

## 🔄 How It Works

### Simple Example

**Scenario**: Chelsea vs Manchester United match

1. **Poll Creation**
   - User creates poll: "Will Palmer score a goal?"
   - Sets lock time: Match kickoff (3:00 PM)

2. **Staking Phase** (Before 3:00 PM)
   - Users stake on "Yes" or "No"
   - Example stakes:
     - Alice: $100 on Yes
     - Bob: $200 on Yes
     - Carol: $150 on Yes
     - Dave: $250 on No
     - Eve: $50 on No
   - **Total Pool**: $750
     - Yes Pool: $450 (60%)
     - No Pool: $300 (40%)

3. **Match Happens**
   - Poll locks at 3:00 PM (no more stakes)
   - Match plays out
   - Palmer scores in the 67th minute! ⚽

4. **Resolution Phase** (After match ends)
   - Voting opens for 2 hours
   - Users who didn't stake on this poll vote on outcome
   - 45 voters say "Yes", 2 say "No" (96% consensus)
   - Automatically approved (>85% threshold)

5. **Payout**
   - Platform takes 5% fee: $750 × 0.05 = **$37.50**
   - Winners split: $750 - $37.50 = **$712.50**
   - Distribution (proportional to stake):
     - Alice: $100/$450 × $712.50 = **$158.33** (Profit: $58.33)
     - Bob: $200/$450 × $712.50 = **$316.67** (Profit: $116.67)
     - Carol: $150/$450 × $712.50 = **$237.50** (Profit: $87.50)
   - Dave and Eve lose their stakes ($300 total to winners)

### Mathematical Formula

```
Individual Payout = (User Stake / Winning Pool Total) × Total Pool × 0.95

Where:
- User Stake = Amount user put on winning side
- Winning Pool Total = All stakes on winning side
- Total Pool = All stakes from both sides
- 0.95 = After 5% platform fee
```

---

## 👥 User Roles

### 1. **Stakers** (Primary Users)
**What they do:**
- Browse upcoming matches
- Stake crypto on prediction polls
- Monitor their active predictions
- Claim winnings after resolution

**Permissions:**
- Create stakes on any active poll
- View their prediction history
- Withdraw winnings

**Restrictions:**
- Cannot stake after lock time
- Cannot change stake once placed
- Cannot vote on polls they staked on

---

### 2. **Poll Creators** (Also Stakers)
**What they do:**
- Create new prediction questions for matches
- Set poll parameters (question, lock time, category)
- Can stake on their own polls

**Permissions:**
- Create unlimited polls (may add limits later)
- Edit polls before first stake (optional feature)

**Restrictions:**
- Must be registered user
- Cannot create duplicate polls
- Polls must meet minimum standards (clear question, valid lock time)

---

### 3. **Voters/Judges** (Community Members)
**What they do:**
- Vote on poll outcomes after matches
- Review evidence (stats, video clips)
- Earn rewards for voting participation

**Permissions:**
- Vote on any poll they didn't stake on
- Challenge outcomes during dispute window

**Restrictions:**
- Cannot vote on polls they participated in
- Must vote within 2-hour window
- One vote per poll

**Rewards:**
- 0.5-1% of total pool divided among all voters
- Example: $1,000 pool, 50 voters = ~$0.10-0.20 per voter

---

### 4. **Administrators** (Platform Team)
**What they do:**
- Verify outcomes when consensus is 60-85%
- Resolve disputes (multi-sig required for <60% consensus)
- Monitor platform for abuse
- Manage featured matches/polls

**Permissions:**
- Override community vote in contentious cases
- Pause polls if issues detected
- Ban malicious users
- Adjust platform parameters (with time-lock)

**Restrictions:**
- Must provide evidence for decisions
- Cannot unilaterally resolve >85% consensus polls
- Actions logged publicly on blockchain
- Multi-sig required for major decisions

---

## 🎮 Core Features

### Feature 1: Browse & Discover
**User Story**: As a user, I want to find interesting predictions to stake on.

**Components:**
- Home page with upcoming matches
- Match detail pages with all polls
- Trending polls (highest stakes, most participants)
- Search/filter by team, league, date
- Categories: Player Events, Team Events, Score Predictions, Fun/Wild

**UI Elements:**
- Match cards with team logos, date/time
- Poll cards showing question, pools, countdown
- Live pool distribution bars
- Participant counts

---

### Feature 2: Stake on Predictions
**User Story**: As a user, I want to put crypto on my prediction.

**Flow:**
1. Click poll → Opens staking modal
2. Choose side (Yes/No)
3. Enter stake amount
4. See potential winnings calculation (real-time)
5. Review pool distribution
6. Connect wallet (if not connected)
7. Confirm transaction
8. See success animation + confirmation

**Calculations to Show:**
- Current pool ratio (e.g., 65% Yes, 35% No)
- Your potential winnings if you win
- ROI percentage
- Platform fee (5%)

**Validations:**
- Sufficient wallet balance
- Poll not locked yet
- Minimum stake amount (e.g., $10)
- Maximum stake (e.g., $10,000 per poll)

---

### Feature 3: Create Polls
**User Story**: As a user, I want to create prediction questions for matches.

**Flow:**
1. Click "Create Poll"
2. Select match from upcoming matches
3. Choose category (dropdown)
4. Write question (text input, 10-150 characters)
5. Set lock time (dropdown: Kickoff, Halftime, 60min, Custom)
6. Preview poll appearance
7. Submit (small transaction fee)

**Categories:**
- Player Event (goals, assists, cards, substitutions)
- Team Event (possession, corners, shots on target)
- Score Prediction (final score, halftime score, goal difference)
- Fun/Wild (manager reactions, weather, random events)

**Validations:**
- Question must be clear and binary (Yes/No answerable)
- Lock time must be before match end
- No duplicate questions for same match

---

### Feature 4: My Dashboard
**User Story**: As a user, I want to track all my predictions in one place.

**Tabs:**

**Active Stakes**
- Polls I've staked on that haven't locked/resolved
- Shows: Match, question, my stake, my side, current pool status, time remaining
- Actions: View details

**Pending Resolution**
- Matches ended, waiting for voting/verification
- Shows: Status (Voting, Admin Review, Dispute)
- Actions: Vote (if eligible), View evidence

**Voting Opportunities**
- Polls I can vote on (didn't participate)
- Shows: Match, question, reward amount, evidence links
- Actions: Cast vote, View evidence

**Completed**
- Historical predictions
- Shows: Win/Loss badge, amount won/lost, ROI, outcome
- Filter: All, Wins, Losses
- Sort: Date, Profit

---

### Feature 5: Community Voting
**User Story**: As a voter, I want to help verify outcomes and earn rewards.

**Flow:**
1. Navigate to "Vote" section
2. See list of matches needing resolution
3. Click poll → Opens voting interface
4. Review:
   - Match context (final score, key events)
   - Poll question
   - Evidence section (stats, video clips, official sources)
   - Current vote tally (optional: show or hide to avoid bias)
5. Cast vote: Yes / No / Unclear
6. Confirm vote
7. See reward confirmation

**Evidence Sources:**
- Official league stats APIs
- Video clip embeds (YouTube, Twitter)
- Match reports from trusted sources
- User-submitted evidence (with moderation)

**Voting Reward Distribution:**
- Fixed percentage of pool (0.5-1%)
- Divided equally among all voters
- Paid out immediately after resolution

---

### Feature 6: Claim Winnings
**User Story**: As a winner, I want to receive my payout.

**Flow:**
1. Poll resolved → Notification appears
2. Navigate to "Completed" tab
3. See winning predictions with "Claim" button
4. Click "Claim"
5. Transaction processes
6. See success animation (confetti, trophy)
7. Winnings added to wallet

**UI Elements:**
- Big "You Won!" celebration screen
- Breakdown: Your stake → Your winnings → Profit
- Transaction receipt
- Share button (social media)

---

### Feature 7: Wallet Integration
**User Story**: As a user, I want to connect my crypto wallet securely.

**Supported Wallets:**
- MetaMask (primary)
- WalletConnect (mobile wallets)
- Coinbase Wallet
- Trust Wallet

**Wallet UI:**
- "Connect Wallet" button (prominent, top-right)
- Modal with wallet options
- Connected state shows:
  - Truncated address (0x742d...5e9f)
  - Balance (ETH + USD equivalent)
  - Network indicator (Ethereum, Polygon, etc.)
- Dropdown menu: Profile, History, Disconnect

**Security:**
- Never store private keys
- Sign transactions only (no direct transfers without user consent)
- Network validation (warn if wrong network)
- Transaction preview before signing

---

## 🏗️ Technical Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────┐
│                      FRONTEND                            │
│  (React + Tailwind + Web3.js/Ethers.js)                 │
│  - User Interface                                        │
│  - Wallet Connection                                     │
│  - Smart Contract Interaction                            │
└────────────────┬────────────────────────────────────────┘
                 │
                 │ Web3 RPC Calls
                 │
┌────────────────▼────────────────────────────────────────┐
│                    BLOCKCHAIN LAYER                      │
│  (Ethereum / Polygon / Base / Arbitrum)                 │
│                                                          │
│  ┌──────────────────┐  ┌──────────────────┐            │
│  │ PredictionMarket │  │  VotingOracle    │            │
│  │   Contract       │◄─┤   Contract       │            │
│  │                  │  │                  │            │
│  │ - Create Polls   │  │ - Community Vote │            │
│  │ - Stake Funds    │  │ - Admin Verify   │            │
│  │ - Claim Wins     │  │ - Resolve Polls  │            │
│  └──────────────────┘  └──────────────────┘            │
│                                                          │
│  ┌──────────────────┐                                   │
│  │   Treasury       │                                   │
│  │   Contract       │                                   │
│  │                  │                                   │
│  │ - Hold Fees      │                                   │
│  │ - Distribute     │                                   │
│  └──────────────────┘                                   │
└────────────────┬────────────────────────────────────────┘
                 │
                 │ Events & State
                 │
┌────────────────▼────────────────────────────────────────┐
│                   BACKEND / INDEXER                      │
│  (Optional - for better UX)                             │
│  - The Graph (index blockchain events)                  │
│  - Cache poll data                                       │
│  - Match data API integration                            │
│  - Push notifications                                    │
└──────────────────────────────────────────────────────────┘
```

### Tech Stack

**Frontend:**
- **Framework**: React 18+ with TypeScript
- **Styling**: Tailwind CSS + custom gaming UI components
- **Web3**: 
  - ethers.js or web3.js (blockchain interaction)
  - wagmi (React hooks for Ethereum)
  - RainbowKit or ConnectKit (wallet connection UI)
- **State Management**: 
  - React Context API (global state)
  - TanStack Query (server state/caching)
- **Charts**: Recharts (pool distribution, stats)
- **Animations**: Framer Motion
- **Icons**: Lucide React
- **Forms**: React Hook Form + Zod validation

**Smart Contracts:**
- **Language**: Solidity 0.8.20+
- **Framework**: Hardhat or Foundry
- **Libraries**: 
  - OpenZeppelin Contracts (security, access control)
  - Chainlink (time automation, potential price feeds)
- **Testing**: Hardhat tests (JavaScript/TypeScript)

**Backend (Optional but Recommended):**
- **Indexer**: The Graph Protocol (index blockchain events)
- **API**: Node.js + Express (match data, caching)
- **Database**: PostgreSQL (cache poll data for faster queries)
- **Match Data**: 
  - API-Football or similar (match schedules, results)
  - Web scraping fallback

**Infrastructure:**
- **Hosting**: Vercel or Netlify (frontend)
- **RPC Provider**: Alchemy or Infura
- **IPFS**: Store evidence links, poll metadata (optional)
- **CDN**: Cloudflare (fast global access)

**DevOps:**
- **Version Control**: Git + GitHub
- **CI/CD**: GitHub Actions
- **Monitoring**: 
  - Sentry (error tracking)
  - Tenderly (smart contract monitoring)
- **Analytics**: Mixpanel or Amplitude (user behavior)

---

## 📜 Smart Contract Specifications

### Contract 1: PredictionMarket.sol

**Purpose**: Main contract for poll creation, staking, and payouts.

**State Variables:**
```solidity
// Mappings
mapping(uint256 => Poll) public polls;
mapping(uint256 => mapping(address => Stake)) public stakes;
mapping(uint256 => PoolAmounts) public poolAmounts;

// Constants
uint256 public constant PLATFORM_FEE_PERCENTAGE = 5; // 5%
uint256 public minimumStake = 0.01 ether;
uint256 public maximumStake = 10 ether;

// Addresses
address public owner;
address public votingOracle;
address public treasury;

// Counters
uint256 public pollCounter;
```

**Structs:**
```solidity
struct Poll {
    uint256 id;
    uint256 matchId;
    address creator;
    string question;
    uint256 lockTime;
    PollStatus status; // Active, Locked, Resolved, Disputed
    bool outcome; // true = Yes wins, false = No wins
    uint256 createdAt;
    uint256 resolvedAt;
}

struct Stake {
    address user;
    uint256 amount;
    bool isYesSide;
    bool claimed;
    uint256 timestamp;
}

struct PoolAmounts {
    uint256 yesPool;
    uint256 noPool;
}

enum PollStatus {
    Active,
    Locked,
    Voting,
    Resolved,
    Disputed,
    Cancelled
}
```

**Key Functions:**

```solidity
// CREATE POLL
function createPoll(
    uint256 _matchId,
    string memory _question,
    uint256 _lockTime
) external returns (uint256 pollId)

// STAKE
function stake(
    uint256 _pollId,
    bool _isYesSide
) external payable

// RESOLVE (called by VotingOracle)
function resolvePoll(
    uint256 _pollId,
    bool _outcome
) external onlyVotingOracle

// CLAIM WINNINGS
function claimWinnings(uint256 _pollId) external

// VIEW FUNCTIONS
function calculateWinnings(uint256 _pollId, address _user) 
    external view returns (uint256)

function getPollDetails(uint256 _pollId) 
    external view returns (Poll memory)

function getUserStake(uint256 _pollId, address _user) 
    external view returns (Stake memory)

function getPoolAmounts(uint256 _pollId) 
    external view returns (uint256 yesPool, uint256 noPool)

// EMERGENCY
function emergencyWithdraw(uint256 _pollId) external
```

**Events:**
```solidity
event PollCreated(uint256 indexed pollId, uint256 matchId, string question, uint256 lockTime);
event StakePlaced(uint256 indexed pollId, address indexed user, uint256 amount, bool isYesSide);
event PollResolved(uint256 indexed pollId, bool outcome);
event WinningsClaimed(uint256 indexed pollId, address indexed user, uint256 amount);
event PollLocked(uint256 indexed pollId);
```

---

### Contract 2: VotingOracle.sol

**Purpose**: Manages community voting and admin verification for poll resolution.

**State Variables:**
```solidity
mapping(uint256 => VotingSession) public votingSessions;
mapping(uint256 => mapping(address => bool)) public hasVoted;
mapping(uint256 => mapping(address => bool)) public hasStaked; // imported from PredictionMarket

uint256 public constant VOTING_WINDOW = 2 hours;
uint256 public constant AUTO_RESOLVE_THRESHOLD = 85; // 85%
uint256 public constant ADMIN_REVIEW_THRESHOLD = 60; // 60%

address[] public admins;
address public predictionMarket;
```

**Structs:**
```solidity
struct VotingSession {
    uint256 pollId;
    uint256 yesVotes;
    uint256 noVotes;
    uint256 totalVoters;
    uint256 votingStartTime;
    uint256 votingEndTime;
    VoteStatus status;
    string evidenceHash; // IPFS hash of evidence
}

enum VoteStatus {
    NotStarted,
    Open,
    AdminReview,
    Resolved,
    Disputed
}
```

**Key Functions:**

```solidity
// START VOTING (called after match ends)
function initiateVoting(uint256 _pollId, string memory _evidenceHash) 
    external onlyAdmin

// CAST VOTE
function castVote(uint256 _pollId, bool _outcome) external

// AUTO RESOLVE (if consensus reached)
function checkAndAutoResolve(uint256 _pollId) internal

// ADMIN VERIFY (for 60-85% consensus)
function adminVerify(uint256 _pollId, bool _outcome, string memory _reasoning) 
    external onlyAdmin

// DISPUTE
function initiateDispute(uint256 _pollId, string memory _reason) 
    external payable

// RESOLVE DISPUTE (multi-sig required)
function resolveDispute(uint256 _pollId, bool _outcome) 
    external onlyMultiSigAdmins

// VIEW FUNCTIONS
function getVotingStats(uint256 _pollId) 
    external view returns (uint256 yesPercent, uint256 noPercent)

function canVote(uint256 _pollId, address _user) 
    external view returns (bool)
```

**Events:**
```solidity
event VotingStarted(uint256 indexed pollId, uint256 endTime);
event VoteCast(uint256 indexed pollId, address indexed voter, bool outcome);
event AutoResolved(uint256 indexed pollId, bool outcome, uint256 consensus);
event AdminVerified(uint256 indexed pollId, bool outcome, address admin);
event DisputeInitiated(uint256 indexed pollId, address disputer);
event DisputeResolved(uint256 indexed pollId, bool outcome);
```

---

### Contract 3: Treasury.sol

**Purpose**: Holds platform fees and manages fund distribution.

**State Variables:**
```solidity
address public owner;
address public predictionMarket;
uint256 public totalFeesCollected;
mapping(address => uint256) public voterRewards;
```

**Key Functions:**

```solidity
// RECEIVE FEES
function depositFees() external payable onlyPredictionMarket

// DISTRIBUTE VOTER REWARDS
function distributeVoterRewards(
    uint256 _pollId,
    address[] memory _voters,
    uint256 _totalReward
) external onlyVotingOracle

// WITHDRAW FEES (owner)
function withdrawFees(uint256 _amount) external onlyOwner

// CLAIM VOTER REWARD
function claimVoterReward() external
```

---

## 🎨 Frontend Requirements

### Design System (Gaming UI)

**Color Palette:**
```css
/* Background */
--bg-primary: linear-gradient(135deg, #0a0e27 0%, #1a1f3a 100%);
--bg-card: rgba(26, 31, 58, 0.6);
--bg-card-hover: rgba(26, 31, 58, 0.8);

/* Accents */
--accent-cyan: #00d9ff;
--accent-green: #39ff14;
--accent-magenta: #ff006e;
--accent-gold: #ffd700;

/* Status Colors */
--success: #39ff14;
--warning: #ffaa00;
--danger: #ff006e;
--info: #00d9ff;

/* Text */
--text-primary: #ffffff;
--text-secondary: #b8c5d6;
--text-muted: #6b7a8f;
```

**Typography:**
```css
/* Headers */
font-family: 'Rajdhani', 'Orbitron', sans-serif;
text-transform: uppercase;
letter-spacing: 0.1em;

/* Body */
font-family: 'Barlow', 'Inter', sans-serif;

/* Numbers */
font-family: 'Roboto Mono', monospace;
```

**Component Patterns:**

**Buttons:**
```css
/* Primary CTA */
.btn-primary {
  background: linear-gradient(135deg, #00d9ff, #00a3cc);
  border: 2px solid #00d9ff;
  box-shadow: 0 0 20px rgba(0, 217, 255, 0.5);
  text-transform: uppercase;
  letter-spacing: 0.1em;
  font-weight: 700;
}

.btn-primary:hover {
  transform: scale(1.05);
  box-shadow: 0 0 30px rgba(0, 217, 255, 0.8);
}
```

**Cards:**
```css
.poll-card {
  background: rgba(26, 31, 58, 0.6);
  backdrop-filter: blur(10px);
  border: 1px solid rgba(0, 217, 255, 0.3);
  border-radius: 12px;
  clip-path: polygon(
    0% 12px, 12px 0%, 
    100% 0%, 100% calc(100% - 12px), 
    calc(100% - 12px) 100%, 0% 100%
  ); /* Clipped corners */
}
```

---

### Page Specifications

#### 1. Home Page

**Sections:**
- Hero (full-screen)
  - Animated particles background
  - Tagline: "PREDICT. STAKE. WIN."
  - Subtitle: "Community-Powered Football Predictions"
  - CTA: "Connect Wallet" + "Browse Matches"
  - Platform stats (animated counters)

- Upcoming Matches (scrollable horizontal)
  - Match cards with team logos
  - Click → Goes to match detail page

- Trending Polls (grid layout)
  - Top 6 polls by total pool value
  - Shows question, pool amounts, participants

- How It Works (3-step visual)
  - Icons with animations
  - Brief explanations

**Animations:**
- Page load: Fade in with stagger
- Stats counters: Count up on scroll into view
- Cards: Float/hover effects
- Background: Slow particle movement

---

#### 2. Match Detail Page

**URL**: `/match/:matchId`

**Layout:**
```
┌─────────────────────────────────────────┐
│          MATCH HEADER                    │
│  Chelsea vs Man United                   │
│  Dec 20, 2024 - 3:00 PM                  │
│  Stamford Bridge                         │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│     [Create Poll Button]                 │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│  ACTIVE POLLS (Grid)                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐│
│  │ Poll 1   │ │ Poll 2   │ │ Poll 3   ││
│  │          │ │          │ │          ││
│  └──────────┘ └──────────┘ └──────────┘│
└─────────────────────────────────────────┘
```

**Poll Card Contains:**
- Question text
- Yes/No pools (with amounts and percentages)
- Pool distribution bar (visual)
- Countdown timer
- Participant count
- "Stake Now" button

---

#### 3. Staking Modal

**Triggered**: Click "Stake Now" on any poll

**Layout:**
```
┌─────────────────────────────────────────┐
│  Will Palmer score a goal?               │
│  Chelsea vs Man United                   │
├─────────────────────────────────────────┤
│  Choose Side:                            │
│  [ YES ]    [ NO ]                       │
│  (toggle buttons)                        │
├─────────────────────────────────────────┤
│  Stake Amount:                           │
│  [___________] ETH                       │
│  Balance: 2.5 ETH                        │
│  [$50] [$100] [$500] [MAX]               │
├─────────────────────────────────────────┤
│  Current Pool:                           │
│  ███████████░░░░░ 65% Yes, 35% No        │
│                                          │
│  Your Potential Winnings:                │
│  0.15 ETH → 0.23 ETH                     │
│  Profit: +0.08 ETH (+53%)                │
│                                          │
│  Platform Fee: 5% on winnings            │
├─────────────────────────────────────────┤
│         [CONFIRM STAKE]                  │
└─────────────────────────────────────────┘
```

**Calculations Update In Real-Time** as user types amount.

---

#### 4. My Dashboard

**Tabs:**
- Active Stakes
- Pending Resolution
- Voting Opportunities
- Completed

**Active Stakes Tab:**
- List of polls user has staked on
- Each item shows:
  - Match + Poll question
  - Your stake + side
  - Current pool status
  - Time until lock
  - "View Details" button

**Pending Resolution Tab:**
- Shows matches that ended, waiting for resolution
- Status badges: "Voting", "Admin Review", "Dispute"

**Voting Opportunities Tab:**
- List of polls user can vote on
- Shows reward amount
- "Vote Now" button

**Completed Tab:**
- Historical predictions
- Win/Loss badges
- Profit/Loss amounts
- Filter: All, Wins, Losses

---

#### 5. Voting Interface

**URL**: `/vote/:pollId`

**Layout:**
```
┌─────────────────────────────────────────┐
│  POLL TO RESOLVE                         │
│  Will Palmer score a goal?               │
│  Chelsea vs Man United (Ended)           │
├─────────────────────────────────────────┤
│  MATCH RESULT                            │
│  Chelsea 2 - 1 Man United                │
│  Palmer scored in 67' ⚽                  │
├─────────────────────────────────────────┤
│  EVIDENCE                                │
│  [Video Clip] [Official Stats] [Report]  │
├─────────────────────────────────────────┤
│  COMMUNITY VOTES (optional)              │
│  45 voted Yes, 2 voted No                │
├─────────────────────────────────────────┤
│  CAST YOUR VOTE                          │
│  [ YES ]  [ NO ]  [ UNCLEAR ]            │
│                                          │
│  Your Reward: 0.002 ETH                  │
├─────────────────────────────────────────┤
│         [SUBMIT VOTE]                    │
└─────────────────────────────────────────┘
```

---

## 🔄 User Flows

### Flow 1: First-Time User Staking

1. User lands on home page (not connected)
2. Sees trending polls, intrigued
3. Clicks "Browse Matches"
4. Sees upcoming Chelsea vs Man United match
5. Clicks match → Match detail page
6. Sees poll: "Will Palmer score?"
7. Clicks "Stake Now"
8. Modal opens → Prompted to connect wallet
9. Clicks "Connect Wallet"
10. Wallet selection modal appears
11. Selects MetaMask
12. MetaMask popup → Confirms connection
13. Returns to staking modal (now connected)
14. Chooses "Yes" side
15. Enters 0.1 ETH stake
16. Sees potential winnings: 0.15 ETH
17. Clicks "Confirm Stake"
18. MetaMask popup → Confirms transaction
19. Transaction processing (spinner)
20. Success! Confetti animation
21. Redirected to "My Dashboard" → Active Stakes
22. Sees new stake listed

---

### Flow 2: Experienced User Voting

1. User logged in (wallet connected)
2. Receives notification: "New voting opportunity"
3. Navigates to "My Dashboard" → Voting Opportunities
4. Sees 3 polls ready to vote on
5. Clicks one: "Will Rashford start?"
6. Voting interface opens
7. Reviews match result (Rashford did start)
8. Watches video clip evidence
9. Checks official lineup stats
10. Community vote shows 40 Yes, 1 No
11. User votes "Yes"
12. Clicks "Submit Vote"
13. Transaction confirms
14. Success message: "Vote recorded! Reward: 0.002 ETH"
15. Returns to dashboard
16. Reward pending in balance

---

### Flow 3: Creating a Poll

1. User on Match Detail page
2. Clicks "Create Poll" button
3. Modal opens with form
4. Step 1: Match auto-selected (Chelsea vs Man United)
5. Step 2: Chooses category "Player Event"
6. Step 3: Types question: "Will Mudryk get a yellow card?"
7. Step 4: Sets lock time: "At Kickoff"
8. Preview shows how poll will appear
9. Clicks "Create Poll"
10. MetaMask popup (small gas fee)
11. Transaction confirms
12. Success! Poll now visible on match page
13. User can immediately stake on their own poll

---

### Flow 4: Claiming Winnings

1. Match ends, poll resolved
2. User receives notification: "You won!"
3. Navigates to "My Dashboard" → Completed
4. Sees winning prediction with green badge
5. Shows: Stake: 0.1 ETH → Winnings: 0.15 ETH (Profit: +0.05 ETH)
6. "Claim" button glowing
7. Clicks "Claim"
8. MetaMask confirms withdrawal transaction
9. Success screen with trophy animation
10. Balance updates in wallet
11. Transaction receipt shown
12. Share button: "Share your win on Twitter!"

---

## 🔒 Security & Compliance

### Smart Contract Security

**Critical Protections:**

1. **Reentrancy Guard**
   - Use OpenZeppelin's `ReentrancyGuard` on all withdrawal functions
   - Prevents attackers from draining funds

2. **Access Control**
   - Use `Ownable` and role-based access
   - Admin functions require multi-sig for critical actions
   - Time-locks on parameter changes

3. **Integer Overflow**
   - Solidity 0.8+ has built-in checks
   - Use SafeMath for extra safety if needed

4. **Frontrunning Protection**
   - Strict lock times enforced on-chain
   - No stakes accepted after lock time (checked via block.timestamp)

5. **Oracle Manipulation**
   - Hybrid system (community + admin) prevents single point of failure
   - Require non-participants to vote
   - Dispute mechanism for contentious cases

6. **Emergency Mechanisms**
   - Pause functionality for detected issues
   - Emergency withdrawal if poll cancelled
   - Dispute window for user recourse

**Audit Requirements:**
- Professional security audit before mainnet launch
- Use audit firms: Trail of Bits, OpenZeppelin, ConsenSys Diligence
- Bug bounty program post-launch

---

### Legal Considerations

**Regulatory Concerns:**
- Prediction markets may be classified as gambling in some jurisdictions
- Need legal opinion on classification in target markets

**Mitigation Strategies:**
1. **Skill-Based Framing**: Emphasize knowledge/skill over chance
2. **No House Edge**: Platform doesn't take opposing positions
3. **Community Resolution**: Peer-to-peer, not platform vs user
4. **Geographic Restrictions**: Block users from prohibited jurisdictions
5. **Age Verification**: Require 18+ (integrate with wallet verification)
6. **Terms of Service**: Clear disclaimers, user assumes risk

**Recommended Actions:**
- Consult with crypto-focused law firm
- Research: Polymarket, Augur, PredictIt case studies
- Consider DAO structure to decentralize control
- Get licenses if operating in regulated markets (UK, US states)

---

### Privacy & Data

**User Data:**
- Wallet addresses are pseudonymous (not anonymous)
- Don't collect personal info unless required by law
- Use IPFS for evidence storage (decentralized)

**GDPR Compliance** (if EU users):
- Right to be forgotten (difficult with blockchain)
- Minimize off-chain data collection
- Privacy policy clearly states blockchain permanence

---

## 🚀 Development Phases

### Phase 1: MVP (3-4 months)

**Goal**: Launch basic functional platform on testnet

**Deliverables:**
1. **Smart Contracts (v1)**
   - PredictionMarket.sol (basic functionality)
   - Admin-only resolution (no voting yet)
   - Deploy to Sepolia/Goerli testnet

2. **Frontend (Core)**
   - Home page
   - Match detail page
   - Staking interface
   - My Dashboard (Active + Completed tabs)
   - Wallet connection (MetaMask only)

3. **Backend**
   - Match data API integration
   - Basic indexing of contract events

4. **Testing**
   - Unit tests for contracts (90%+ coverage)
   - Frontend E2E tests (Cypress)
   - Internal user testing (10-20 users)

**Success Metrics:**
- 50+ test stakes placed
- 10+ matches with polls
- Zero critical bugs
- Positive user feedback

---

### Phase 2: Community Features (2-3 months)

**Goal**: Add voting system and improve UX

**Deliverables:**
1. **Smart Contracts (v2)**
   - VotingOracle.sol implementation
   - Community voting mechanics
   - Dispute resolution
   - Treasury contract

2. **Frontend (Enhanced)**
   - Voting interface
   - Poll creation by users
   - Voting Opportunities tab
   - Evidence display
   - Improved animations/UI polish

3. **Backend**
   - Video clip embedding
   - Stats API integration
   - Push notifications (browser)

4. **Testing**
   - Security audit (preliminary)
   - Closed beta (100 users)

**Success Metrics:**
- 80%+ auto-resolution rate (consensus)
- <5% disputed polls
- Avg 20+ voters per poll

---

### Phase 3: Public Beta (2 months)

**Goal**: Launch on mainnet with real money

**Deliverables:**
1. **Security**
   - Professional smart contract audit
   - Fix all findings
   - Bug bounty program launch

2. **Frontend**
   - Multi-wallet support (WalletConnect, Coinbase)
   - Mobile app (React Native or PWA)
   - Advanced analytics dashboard
   - Social features (leaderboards, profiles)

3. **Marketing**
   - Landing page
   - Documentation/FAQ
   - Community Discord/Telegram
   - Influencer partnerships

4. **Operations**
   - Customer support system
   - Admin dashboard for moderation
   - Legal compliance (terms, privacy)

**Launch Strategy:**
- Soft launch: 1-2 featured matches
- Invite-only first 1,000 users
- Gradual rollout over 4 weeks
- Monitor for issues, quick fixes

**Success Metrics:**
- 1,000+ registered wallets
- $50K+ total volume
- <1% error rate
- 4.0+ star user rating

---

### Phase 4: Scale & Optimize (Ongoing)

**Goal**: Grow user base and add features

**Roadmap Ideas:**
1. **Multi-Sport Expansion**
   - Basketball (NBA)
   - American Football (NFL)
   - Cricket, Tennis, etc.

2. **Advanced Features**
   - Combo predictions (multiple events)
   - Live in-game predictions
   - Peer-to-peer challenges
   - Prediction pools (group stakes)

3. **Gamification**
   - User levels/XP
   - Achievement badges
   - Seasonal leaderboards
   - Referral rewards

4. **DAO Governance**
   - Issue governance token
   - Community votes on features
   - Fee distribution to token holders

5. **Layer 2 Migration**
   - Move to Polygon/Arbitrum for lower fees
   - Cross-chain support

6. **Mobile Apps**
   - Native iOS/Android apps
   - Push notifications for match starts, results

---

## 📊 Success Metrics & KPIs

### Platform Health
- **Total Value Locked (TVL)**: $ staked across all active polls
- **Active Users**: Monthly active wallets
- **Poll Volume**: # of polls created per week
- **Average Pool Size**: Mean total stakes per poll
- **Platform Fees Collected**: Monthly revenue

### User Engagement
- **Retention Rate**: % users who stake 2+ times
- **Voting Participation**: % eligible voters who vote
- **Poll Creation Rate**: % users who create at least one poll
- **Time on Platform**: Average session duration
- **Referral Rate**: % users who invite others

### Resolution Accuracy
- **Auto-Resolution Rate**: % polls resolved via consensus (>85%)
- **Dispute Rate**: % polls that go to dispute
- **Admin Override Rate**: % polls requiring admin decision
- **Average Resolution Time**: Hours from match end to payout

### Quality Metrics
- **Error Rate**: % transactions that fail
- **Uptime**: % platform availability
- **User Satisfaction**: NPS score
- **Support Tickets**: # issues reported per week

---

## 📝 Glossary

**Terms for Developers:**

- **Poll**: A prediction question with binary outcome (Yes/No)
- **Stake**: Amount of crypto a user puts on one side of a poll
- **Pool**: Total amount staked on one side (Yes Pool vs No Pool)
- **Lock Time**: Deadline when poll closes to new stakes
- **Resolution**: Process of determining poll outcome
- **Oracle**: System that determines truth (in our case: voting + admin)
- **Consensus**: Agreement threshold for auto-resolution (85%)
- **Dispute**: Formal challenge to a poll outcome
- **Platform Fee**: Percentage taken from winning pool (5%)
- **Proportional Payout**: Winners split pot based on stake size
- **Voter Reward**: Small payment to community members who vote

**Example Calculation:**
```
Total Pool: $1,000
- Yes Pool: $700 (70%)
- No Pool: $300 (30%)

Outcome: Yes wins

Platform Fee: $1,000 × 5% = $50
Winners Split: $1,000 - $50 = $950

Alice staked $140 on Yes (20% of Yes Pool):
Alice's Payout: ($140 / $700) × $950 = $190
Alice's Profit: $190 - $140 = $50
```

---

## 🎯 Next Steps for Development Team

### Immediate Actions (Week 1):
1. Set up development environment
   - Initialize Git repository
   - Install Hardhat/Foundry
   - Set up React project with Tailwind

2. Define project structure
   - Frontend folder structure
   - Smart contract file organization
   - Testing framework setup

3. Create mock data
   - Sample matches
   - Sample polls
   - Test user data

4. Design database schema (if using backend)
   - Tables for cached data
   - Indexer structure

### Sprint 1 (Weeks 2-4):
- **Smart Contracts**: Write PredictionMarket.sol (basic version)
- **Frontend**: Build home page + wallet connection
- **Testing**: Unit tests for contract functions

### Sprint 2 (Weeks 5-7):
- **Smart Contracts**: Add staking and payout logic
- **Frontend**: Build match detail page + staking modal
- **Integration**: Connect frontend to testnet contracts

### Sprint 3 (Weeks 8-10):
- **Smart Contracts**: Implement VotingOracle.sol
- **Frontend**: Build My Dashboard + voting interface
- **Testing**: Integration tests, user testing

---

## 📞 Questions for Stakeholders

Before development begins, clarify:

1. **Target Blockchain**: Ethereum mainnet? Layer 2 (Polygon, Arbitrum)? Multiple chains?
2. **Budget**: Development costs, audit costs, infrastructure costs?
3. **Timeline**: Hard launch date? Phased rollout?
4. **Legal**: Do we have legal counsel? Which jurisdictions are we targeting?
5. **Team**: Who's on the team? Developers, designers, marketers?
6. **Competitive Analysis**: Who are our main competitors? What's our differentiation?
7. **Revenue Model**: Just platform fees? Future token launch? Other monetization?

---

## 📚 Additional Resources

**Learn Solidity:**
- CryptoZombies: https://cryptozombies.io
- Solidity by Example: https://solidity-by-example.org
- OpenZeppelin Docs: https://docs.openzeppelin.com

**Web3 Frontend:**
- wagmi Documentation: https://wagmi.sh
- RainbowKit: https://www.rainbowkit.com
- ethers.js: https://docs.ethers.org

**Security:**
- Smart Contract Security Best Practices: https://consensys.github.io/smart-contract-best-practices
- Trail of Bits Security Guides: https://github.com/crytic/building-secure-contracts

**Inspiration:**
- Polymarket (prediction market): https://polymarket.com
- Augur (decentralized oracle): https://augur.net
- PredictIt (regulated prediction market): https://www.predictit.org

---

**END OF SPECIFICATION**

*Last Updated: [Current Date]*  
*Version: 1.0*  
*Contributors: Product Team*