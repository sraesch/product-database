import { Typography } from "@mui/material";
import AppBar from "@mui/material/AppBar";
import Toolbar from "@mui/material/Toolbar";
import Navigation from "./Navigation";
import { routes } from "../routes";

export default function MenuBar() {
    return (
        <AppBar position="static">
            <div style={{ display: 'flex', justifyContent: 'start', padding: '0 1rem' }}>
                <Toolbar disableGutters style={{ width: '100%' }}>
                    <div style={{ display: 'flex', flexDirection: 'row', justifyContent: 'start', width: '100%' }}>
                        <div style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', flexGrow: 1 }}>
                            <img src="/app.png" style={{ width: '3rem' }} />
                            <Typography variant="h5" component="div" sx={{ flexGrow: 1, marginLeft: '1rem' }} >
                                Product Database
                            </Typography>
                        </div>
                        <Navigation entries={routes} />
                    </div>
                </Toolbar>
            </div>
        </AppBar >
    );
}