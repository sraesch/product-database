import { useState } from 'react';
import './App.scss'
import AppBar, { AppBarMenuEntry } from './components/AppBar'

const sections: AppBarMenuEntry[] = [
  {
    id: 'products',
    label: 'Products',
  },
  {
    id: 'requests',
    label: 'Product Requests',
  }
];


function App() {
  const [activeSection, setActiveSection] = useState('products');

  return (
    <div className="App">
      <header>
        <AppBar activeMenu={activeSection} onMenuClick={setActiveSection} menuEntries={sections} title='Product DB' />
      </header>
      <main>
      </main>
    </div>
  )
}

export default App
