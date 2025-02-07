import logo from './logo.png';
import './AppBar.scss';
import { useState } from 'react';

interface AppBarMenuProps {
    /**
     * The menu entries of the app bar.
     */
    menuEntries: AppBarMenuEntry[];

    /**
     * The active menu.
     */
    activeMenu: string;

    /**
     * The callback function when a menu is clicked.
     */
    onMenuClick?: (menuId: string) => void;
}

function AppBarMenu(props: AppBarMenuProps) {
    const [activeMenu, setActiveMenu] = useState(props.activeMenu);

    const handleOnClick = (e: React.MouseEvent<HTMLButtonElement>) => {
        const target = e.target as HTMLButtonElement;
        setActiveMenu(target.id);

        if (props.onMenuClick) {
            props.onMenuClick(target.id);
        }
    };

    return (
        <div className="app-bar-menu">
            {props.menuEntries.map((entry) => (
                <button key={entry.id} id={entry.id} className={activeMenu == entry.id ? "app-bar-menu-button-enabled" : "app-bar-menu-button"} onClick={handleOnClick}>
                    {entry.label}
                </button>
            ))}
        </div>
    )
}


/**
 * A menu entry of the AppBar component.
 */
export interface AppBarMenuEntry {
    /**
     * The id of the menu entry.
     */
    id: string;

    /**
     * The label of the menu entry.
     */
    label: string;
}

/**
 * The properties of the AppBar component.
 */
export interface AppBarProps {
    /**
     * The title of the app bar.
     */
    title: string;

    /**
     * The menu entries of the app bar.
     */
    menuEntries: AppBarMenuEntry[];

    /**
     * The active menu.
     */
    activeMenu: string;

    /**
     * The callback function when a menu is clicked.
     */
    onMenuClick?: (menuId: string) => void;
}

export default function AppBar(props: AppBarProps) {
    return (
        <div className="app-bar">
            <img src={logo} alt="logo" className="app-bar-logo" />
            <h2 className="app-bar-title">
                {props.title}
            </h2>
            <AppBarMenu activeMenu={props.activeMenu} menuEntries={props.menuEntries} onMenuClick={props.onMenuClick} />
        </div>
    )
}