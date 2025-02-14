import './App.css'

import MenuBar from './components/MenuBar';
import { Route, BrowserRouter as Router, Routes } from 'react-router';
import Products from './components/Products';
import { JSX } from 'react';
import { routes } from './routes';


function App(): JSX.Element {

  return (
    <Router>
      <div className='App'>
        <header>
          <MenuBar />
        </header>
        <main>

          <Routes>
            <Route path="/" element={<Products />} />
            {
              routes.map((route, index) => (
                <Route key={index} path={route.route} element={route.component} />
              ))
            }
          </Routes>
        </main>
      </div>
    </Router>
  )
}

export default App;
