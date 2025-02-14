import { Button } from "@mui/material";
import { JSX } from "react";
import { /*matchPath, useLocation, Location,*/ Link } from "react-router";

/**
 * A single entry in the navigation bar.
 */
export interface NavigationEntry {
    label: string;
    route: string;
}

export interface TabsNavigationProps {
    /**
     * The entries to display in the navigation bar.
     */
    entries: NavigationEntry[];
}

// /**
//  * Determines the index of the route that matches the current location.
//  * 
//  * @param entries The entries to check.
//  * @param location The current location.
//  * 
//  * @returns The index of the matching route or 0 if no match is found.
//  */
// function useRouteMatch(entries: NavigationEntry[], location: Location): number {
//     const { pathname } = location;

//     for (let i = 0; i < entries.length; i += 1) {
//         const pattern = entries[i].route;
//         const possibleMatch = matchPath(pattern, pathname);
//         if (possibleMatch !== null) {
//             return i;
//         }
//     }

//     return 0;
// }

export default function TabsNavigation(props: TabsNavigationProps): JSX.Element {
    // const location: Location = useLocation();
    // const currentTab = useRouteMatch(props.entries, location);

    return (
        <div style={{ display: 'flex', flexDirection: 'row', justifyContent: 'center' }}>
            {
                props.entries.map((entry, index) => (
                    <Button color="inherit" key={index} component={Link} to={entry.route}>
                        {entry.label}
                    </Button>
                ))
            }
        </div>
    );
}
