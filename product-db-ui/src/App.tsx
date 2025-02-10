import { useTheme } from '@mui/material';
import Tabs from '@mui/material/Tabs';
import Tab from '@mui/material/Tab';
import './App.css'

import MenuBar from './components/MenuBar';
import { useState } from 'react';
import { Route, BrowserRouter as Router, Routes } from 'react-router-dom';
import Products from './components/Products';
import ProductRequests from './components/ProductRequests';

function App() {
  const theme = useTheme();
  const [value, setValue] = useState<string>('products');

  const handleChange = (_: React.SyntheticEvent, newValue: string) => {
    setValue(newValue);
  };

  return (
    <Router>
      <div className='App'>
        <header>
          <MenuBar />
        </header>
        <main>
          <Tabs style={{
            backgroundColor: theme.palette.primary.main,
            color: theme.palette.text.disabled
          }} variant="fullWidth" value={value} onChange={handleChange}>
            <Tab label="Products" value="products" />
            <Tab label="Product Requests" value="requests" />
          </Tabs>
          <Routes>
            <Route path="/" element={<Products />} />
            <Route path="/products" element={<Products />} />
            <Route path="/requests" element={<ProductRequests />} />
          </Routes>
        </main>
      </div>
    </Router>
  )
}

export default App;
