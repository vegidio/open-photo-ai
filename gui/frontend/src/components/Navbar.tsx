import { NavbarBrand, NavbarCollapse, NavbarLink, Navbar as Navigation } from 'flowbite-react';

type NavbarProps = {
    className?: string;
};

export const Navbar = ({ className = '' }: NavbarProps) => {
    return (
        <Navigation fluid={true} className={`bg-[#212121] ${className}`}>
            <NavbarBrand>
                <span className="self-center whitespace-nowrap text-lg font-semibold dark:text-[#b0b0b0]">
                    Open Photo AI
                </span>
            </NavbarBrand>

            <NavbarCollapse>
                <NavbarLink href="/">Home</NavbarLink>
                <NavbarLink href="/about">About</NavbarLink>
                <NavbarLink href="/docs/components/navbar">Navbar</NavbarLink>
                <NavbarLink href="/pricing">Pricing</NavbarLink>
                <NavbarLink href="/contact">Contact</NavbarLink>
            </NavbarCollapse>
        </Navigation>
    );
};
