import { Navbar } from './components/Navbar';
import { Preview } from './components/Preview';
import { Sidebar } from './components/Sidebar';

export const Home = () => {
    return (
        <div className="flex h-screen flex-col">
            <Navbar />

            <main className="flex flex-1 flex-row">
                <Preview className="flex-1" />

                <Sidebar className="w-64" />
            </main>
        </div>
    );
};
