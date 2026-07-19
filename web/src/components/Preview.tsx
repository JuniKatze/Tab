interface PreviewProps {
  html: string;
  error: string | null;
  loading: boolean;
}

export default function Preview({ html, error, loading }: PreviewProps) {
  if (loading) {
    return (
      <div className="preview-loading">
        <p>Rendering...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="preview-error">
        <h3>Render Error</h3>
        <pre>{error}</pre>
      </div>
    );
  }

  return (
    <div
      className="preview-scroll"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
