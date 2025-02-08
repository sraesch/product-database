import { Typography } from "@mui/material";
import AppBar from "@mui/material/AppBar";
import Container from "@mui/material/Container";
import Toolbar from "@mui/material/Toolbar";

export default function MenuBar() {
    return (
        <AppBar position="static">
            <Container maxWidth="xl">
                <Toolbar disableGutters>
                    <img src="/app.png" style={{ width: '3.5rem' }} />
                    <Typography variant="h5" component="div" sx={{ flexGrow: 1, marginLeft: '1rem' }} >
                        Product Database
                    </Typography>
                </Toolbar>
            </Container>
        </AppBar >
    );
}