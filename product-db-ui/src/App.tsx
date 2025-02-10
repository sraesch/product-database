import { useTheme } from '@mui/material';
import Tabs from '@mui/material/Tabs';
import Tab from '@mui/material/Tab';
import './App.css'

import MenuBar from './components/MenuBar';
import { Link, Location, matchPath, Route, BrowserRouter as Router, Routes, useLocation } from 'react-router';
import Products from './components/Products';
import ProductRequests from './components/ProductRequests';
import { JSX } from 'react';

const routes = ['/products', '/requests'];


function useRouteMatch(location: Location): number {
  const { pathname } = location;

  for (let i = 0; i < routes.length; i += 1) {
    const pattern = routes[i];
    const possibleMatch = matchPath(pattern, pathname);
    if (possibleMatch !== null) {
      return i;
    }
  }

  return 0;
}

function TabsNavigation(): JSX.Element {
  const theme = useTheme();
  const location: Location = useLocation();
  const currentTab = useRouteMatch(location);

  return (
    <Tabs value={currentTab} sx={{
      backgroundColor: theme.palette.primary.main,
      ".Mui-selected": {
        color: 'white',
      },
    }} variant="fullWidth">
      <Tab label="Products" component={Link} to="/products" />
      <Tab label="Product Requests" component={Link} to="/requests"  >
      </Tab>
    </Tabs>
  );
}


function App() {

  return (
    <Router>
      <div className='App'>
        <header>
          <MenuBar />
        </header>
        <main>
          <TabsNavigation />
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
