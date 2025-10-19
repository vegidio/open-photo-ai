export const PreviewImageSideBySide = () => {
    return (
        <div className="flex flex-row h-full w-full p-2 gap-2">
            <div className="flex-1 flex items-center justify-center">
                <img src="https://picsum.photos/1920/1080" className="max-w-full max-h-full" />
            </div>

            <div className="flex-1 flex items-center justify-center">
                <img src="https://picsum.photos/1920/1080" className="max-w-full max-h-full" />
            </div>
        </div>
    );
};
