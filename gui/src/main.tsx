import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { ColorblindProvider } from './hooks/useColorblind.tsx'
import { injectTokens } from './theme.ts'

injectTokens()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ColorblindProvider>
      <App />
    </ColorblindProvider>
  </StrictMode>,
)
