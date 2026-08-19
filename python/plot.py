#!/usr/bin/env python3

'''Methods for making plots of embeddings'''

import argparse
from collections import defaultdict
import pandas as pd
import numpy as np
import plotly.express as px
import plotly.graph_objects as go

import matplotlib as mpl
mpl.use('Agg')
#mpl.rcParams.update({'font.size': 8})
import matplotlib.pyplot as plt
from mpl_toolkits.axes_grid1 import make_axes_locatable


def _read_names(names_path):
    """Read one sample name per line from *names_path*."""
    with open(names_path, encoding='utf-8') as names_file:
        names = [line.rstrip('\r\n') for line in names_file]

    if not names:
        raise ValueError(f"names file is empty: {names_path}")
    if any(not name for name in names):
        raise ValueError(f"names file contains an empty sample name: {names_path}")
    if len(names) != len(set(names)):
        raise ValueError(f"names file contains duplicate sample names: {names_path}")
    return names


def _read_embedding(embedding_path, n_names):
    """Read and validate a two-dimensional embedding."""
    try:
        embedding = np.loadtxt(embedding_path, ndmin=2)
    except ValueError as error:
        raise ValueError(f"could not read embedding file {embedding_path}: {error}") from error

    if embedding.ndim != 2 or embedding.shape[1] != 2:
        raise ValueError(
            f"embedding must have two columns: {embedding_path}"
        )
    if embedding.shape[0] != n_names:
        raise ValueError(
            f"embedding/name count mismatch: {embedding_path} has "
            f"{embedding.shape[0]} rows, names file has {n_names}"
        )
    return embedding


def _read_label_tsv(labels_path, names):
    """Read unheadered ``sample_name<TAB>label`` rows in names order."""
    labels_by_name = {}
    with open(labels_path, encoding='utf-8') as labels_file:
        for line_number, line in enumerate(labels_file, start=1):
            fields = line.rstrip('\r\n').split('\t')
            if len(fields) != 2:
                raise ValueError(
                    f"label file row {line_number} must contain exactly two "
                    "tab-separated fields"
                )
            sample_name, label = fields
            if not sample_name:
                raise ValueError(f"label file row {line_number} has an empty sample name")
            if sample_name in labels_by_name:
                raise ValueError(
                    f"label file contains duplicate sample name: {sample_name}"
                )
            labels_by_name[sample_name] = label

    name_set = set(names)
    label_set = set(labels_by_name)
    if label_set != name_set:
        missing = sorted(name_set - label_set)
        extra = sorted(label_set - name_set)
        details = []
        if missing:
            details.append(f"missing names: {', '.join(missing)}")
        if extra:
            details.append(f"unknown names: {', '.join(extra)}")
        raise ValueError("label/name mismatch (" + "; ".join(details) + ")")

    return [labels_by_name[name] for name in names]


def _load_plot_inputs(input_prefix, labels_path=None):
    """Load an embedding, names, and optionally user-supplied labels."""
    names_path = input_prefix + '.names.txt'
    embedding_path = input_prefix + '.embedding.txt'
    names = _read_names(names_path)
    embedding = _read_embedding(embedding_path, len(names))
    labels = None if labels_path is None else _read_label_tsv(labels_path, names)
    return embedding, names, labels


def main(argv=None):
    """Load an embedding and create the HTML, density, and static plots."""
    parser = argparse.ArgumentParser(
        description='Plot a Mandrake embedding from an input prefix.'
    )
    parser.add_argument(
        'input_prefix',
        help='prefix for <prefix>.embedding.txt and <prefix>.names.txt',
    )
    label_source = parser.add_mutually_exclusive_group(required=True)
    label_source.add_argument(
        '--labels',
        metavar='LABELS.tsv',
        help='unheadered sample-name<TAB>label file',
    )
    label_source.add_argument(
        '--hdbscan',
        action='store_true',
        help='generate labels with HDBSCAN and write the cluster table',
    )
    args = parser.parse_args(argv)

    try:
        embedding, names, labels = _load_plot_inputs(
            args.input_prefix,
            labels_path=args.labels,
        )
        if args.hdbscan:
            labels = runHDBSCAN(embedding)
            if len(labels) != len(names):
                raise ValueError('HDBSCAN returned a label for the wrong number of samples')
            write_hdbscan_clusters(labels, names, args.input_prefix)

        dbscan = args.hdbscan
        plotSCE_html(
            embedding,
            names,
            labels,
            args.input_prefix,
            dbscan=dbscan,
        )
        plotSCE_hex(embedding, args.input_prefix)
        plotSCE_mpl(
            embedding,
            labels,
            args.input_prefix,
            dbscan=dbscan,
        )
    except (ImportError, OSError, UnicodeError, ValueError) as error:
        parser.error(str(error))

def norm_and_centre(array):
    means = np.mean(array, axis=0)
    array -= means
    scales = np.std(array, axis=0)
    array /= scales

# For HDBSCAN
def _scale_and_centre(array):
    means = np.mean(array, axis=0)
    array_scaled = array - means
    scales = 0.5 * (np.max(array_scaled, axis=0) -
                    np.min(array_scaled, axis=0))
    array_scaled /= scales
    return(array_scaled)

def runHDBSCAN(embedding):
    import hdbscan
    embedding_scaled = _scale_and_centre(embedding)
    hdb = hdbscan.HDBSCAN(algorithm='boruvka_balltree',
                          min_cluster_size=2,
                          min_samples=2,
                          cluster_selection_epsilon=0.02,
                          allow_single_cluster=True
                          ).fit(embedding_scaled)
    return hdb.labels_


def write_hdbscan_clusters(clusters, labels, output_prefix):
    d = defaultdict(list)
    for label, cluster in zip(labels, clusters):
        d['id'].append(label)
        d['hdbscan_cluster__autocolour'].append(cluster)
    pd.DataFrame(data=d).to_csv(output_prefix + ".embedding_hdbscan_clusters.csv",
                                index=False)

# Interactive HTML plot using plotly
def plotSCE_html(embedding, names, labels, output_prefix, hover_labels=True, dbscan=True, seed=42):
    if dbscan:
        not_noise = labels != -1
        not_noise_list = list(np.where(not_noise)[0])
        plot_df = pd.DataFrame({'SCE dimension 1': embedding[not_noise, 0],
                                'SCE dimension 2': embedding[not_noise, 1],
                                'names': [names[i] for i in not_noise_list],
                                'Label': [str(labels[x]) for x in not_noise_list]})
    else:
        plot_df = pd.DataFrame({'SCE dimension 1': embedding[:, 0],
                                'SCE dimension 2': embedding[:, 1],
                                'names': names,
                                'Label': [str(x) for x in labels]})

    random_colour_map = {}
    rng = np.random.default_rng(seed=seed)
    for label in sorted(pd.unique(plot_df['Label'])):
        # Alternative approach with hsl representation
        # from hsluv import hsluv_to_hex ## outside of loop
        # hue = rng.uniform(0, 360)
        # saturation = rng.uniform(60, 100)
        # luminosity = rng.uniform(50, 90)
        # random_colour_map[label] = hsluv_to_hex([hue, saturation, luminosity])

        # Random in rbg seems to give better contrast
        rgb = rng.integers(low=0, high=255, size=3)
        random_colour_map[label] = ",".join(["rgb(" + str(rgb[0]),
                                              str(rgb[1]),
                                              str(rgb[2]) + ")"])

    # Plot clustered points
    fig = px.scatter(plot_df, x="SCE dimension 1", y="SCE dimension 2",
                     hover_name='names' if hover_labels else None,
                     color='Label',
                     color_discrete_map=random_colour_map,
                     render_mode='webgl')
    fig.layout.update(showlegend=False)
    fig.update_traces(marker=dict(size=10,
                             line=dict(width=2,
                                       color='DarkSlateGrey')),
                      text=plot_df['names'] if hover_labels else None,
                      hoverinfo='text' if hover_labels else None,
                      opacity=1.0,
                      selector=dict(mode='markers'))
    if dbscan:
        # Plot noise points
        fig.add_trace(
            go.Scattergl(
                mode='markers',
                x=embedding[labels == -1, 0],
                y=embedding[labels == -1, 1],
                text=[names[i] for i in list(np.where(labels == -1)[0])] if hover_labels else None,
                hoverinfo='text' if hover_labels else None,
                opacity=0.5,
                marker=dict(
                    color='black',
                    size=8
                ),
                showlegend=False
            )
        )

    fig.write_html(output_prefix + '.embedding.html')

# Hexagon density plot to see overplotting
def plotSCE_hex(embedding, output_prefix):
    # Set up figure with scale bar
    plt.figure(figsize=(8, 8), dpi=320, facecolor='w', edgecolor='k')
    ax = plt.subplot()
    divider = make_axes_locatable(ax)
    cax = divider.append_axes("right", size="5%", pad=0.05)

    # Hex plot
    hb = ax.hexbin(embedding[:, 0], embedding[:, 1],
                   mincnt=1, gridsize=50, cmap='inferno')

    # Colour bar
    cbar = plt.colorbar(hb, cax=cax)
    cbar.set_label('Samples')

    # Draw the plot
    ax.set_title('Embedding density')
    ax.set_xlabel('SCE dimension 1')
    ax.set_ylabel('SCE dimension 2')
    plt.savefig(output_prefix + ".embedding_density.pdf")

# Matplotlib static plot, and animation if available
def plotSCE_mpl(embedding, labels, output_prefix,
                dbscan=True, seed=4000):
    # Set the style by group
    if embedding.shape[0] > 10000:
        pt_scale = 1.5
    elif embedding.shape[0] > 1000:
        pt_scale = 3
    else:
        pt_scale = 7

    # If labels are strings
    unique_labels = set(labels)
    if not isinstance(labels, np.ndarray):
        labels = np.array(labels, dtype="object")

    rng = np.random.default_rng(seed=seed)
    style_dict = defaultdict(dict)
    for k in sorted(unique_labels):
        if k == -1 and dbscan:
            style_dict['ptsize'][k] = 1 * pt_scale
            style_dict['col'][k] = 'k'
            style_dict['mec'][k] = None
            style_dict['mew'][k] = 0
        else:
            style_dict['ptsize'][k] = 2 * pt_scale
            style_dict['col'][k] = tuple(rng.uniform(size=3))
            style_dict['mec'][k] = 'k' if embedding.shape[0] <= 10000 else None
            style_dict['mew'][k] = 0.2 * pt_scale if embedding.shape[0] <= 10000 else 0

    # Static figure is a scatter plot, drawn by class
    plt.figure(figsize=(8, 8), dpi=320, facecolor='w', edgecolor='k')
    cluster_sizes = {}
    for k in sorted(unique_labels):
        class_member_mask = (labels == k)
        xy = embedding[class_member_mask]
        cluster_sizes[k] = xy.shape[0]
        plt.plot(xy[:, 0], xy[:, 1], '.',
                 color=style_dict['col'][k],
                 markersize=style_dict['ptsize'][k],
                 mec=style_dict['mec'][k],
                 mew=style_dict['mew'][k])

    # plot output
    if dbscan:
        plt.title('HDBSCAN – estimated number of spatial clusters: %d' % (len(unique_labels) - 1))
    plt.xlabel('SCE dimension 1')
    plt.ylabel('SCE dimension 2')
    plt.savefig(output_prefix + ".embedding_static.png")
    plt.close()


if __name__ == '__main__':
    main()
