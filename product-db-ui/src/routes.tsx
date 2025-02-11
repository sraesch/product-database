import ProductRequests from "./components/ProductRequests";
import Products from "./components/Products";

export const routes = [
    {
        label: 'Products',
        route: 'products',
        component: <Products />
    },
    {
        label: 'Requests',
        route: 'requests',
        component: <ProductRequests />
    }
];
